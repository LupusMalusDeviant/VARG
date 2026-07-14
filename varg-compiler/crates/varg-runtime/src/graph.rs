// Wave 20 + Issue #3: Native Knowledge Graph Runtime with SQLite Persistence
//
// Embedded graph engine using adjacency lists with write-through SQLite storage.
// On graph_open(name), opens {name}.graph.db — loads existing data into memory.
// All mutations (add_node, add_edge) are written through to SQLite immediately.
// Falls back to pure in-memory if name is ":memory:".

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: u64,
    pub label: String,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from: u64,
    pub to: u64,
    pub relation: String,
    pub properties: HashMap<String, String>,
}

pub struct GraphDb {
    pub name: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// R1: next node ID for THIS graph. Was a global `static NODE_COUNTER`, which meant two
    /// graph instances shared one ID sequence (IDs from one graph leaked into another and
    /// interacted badly with reload). Per-instance counting keeps each graph self-contained.
    next_id: u64,
    /// SQLite connection for persistence (None = pure in-memory)
    db: Option<Connection>,
}

// Manual Debug since Connection doesn't implement Debug
impl std::fmt::Debug for GraphDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphDb")
            .field("name", &self.name)
            .field("nodes", &self.nodes)
            .field("edges", &self.edges)
            .field("db", &self.db.is_some())
            .finish()
    }
}

/// Shared, thread-safe graph handle
pub type GraphHandle = Arc<Mutex<GraphDb>>;

/// Initialize SQLite schema for graph persistence
fn init_graph_schema(conn: &Connection) {
    if let Err(e) = conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS graph_nodes (
            id INTEGER PRIMARY KEY,
            label TEXT NOT NULL,
            properties TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS graph_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_id INTEGER NOT NULL,
            to_id INTEGER NOT NULL,
            relation TEXT NOT NULL,
            properties TEXT NOT NULL
        );"
    ) {
        // R4: don't abort the process on schema init failure — report and continue (the graph
        // will operate in-memory-only if the tables are unavailable).
        eprintln!("[VargOS] graph schema init failed: {}", e);
    }
}

/// Load existing nodes and edges from SQLite into memory.
/// R4: previously every DB read used `unwrap()`, so a corrupt or partially-written database
/// aborted the whole process. Now a failed query returns empty (fresh graph) and individual
/// malformed rows are skipped rather than crashing.
fn load_graph_from_db(conn: &Connection) -> (Vec<GraphNode>, Vec<GraphEdge>, u64) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut max_id: u64 = 0;

    // Load nodes — tolerate query/row failures.
    if let Ok(mut stmt) = conn.prepare("SELECT id, label, properties FROM graph_nodes") {
        let node_rows = stmt.query_map([], |row| {
            let id: u64 = row.get::<_, i64>(0)? as u64;
            let label: String = row.get(1)?;
            let props_json: String = row.get(2)?;
            let properties: HashMap<String, String> = serde_json::from_str(&props_json).unwrap_or_default();
            Ok((id, label, properties))
        });
        if let Ok(rows) = node_rows {
            for row in rows.flatten() {
                let (id, label, properties) = row;
                if id > max_id { max_id = id; }
                nodes.push(GraphNode { id, label, properties });
            }
        }
    }

    // Load edges — tolerate query/row failures.
    if let Ok(mut stmt) = conn.prepare("SELECT from_id, to_id, relation, properties FROM graph_edges") {
        let edge_rows = stmt.query_map([], |row| {
            let from: u64 = row.get::<_, i64>(0)? as u64;
            let to: u64 = row.get::<_, i64>(1)? as u64;
            let relation: String = row.get(2)?;
            let props_json: String = row.get(3)?;
            let properties: HashMap<String, String> = serde_json::from_str(&props_json).unwrap_or_default();
            Ok((from, to, relation, properties))
        });
        if let Ok(rows) = edge_rows {
            for row in rows.flatten() {
                let (from, to, relation, properties) = row;
                edges.push(GraphEdge { from, to, relation, properties });
            }
        }
    }

    (nodes, edges, max_id)
}

/// Open or create a named graph database
/// If name is ":memory:", uses pure in-memory mode.
/// Otherwise, persists to {name}.graph.db
pub fn __varg_graph_open(name: &str) -> GraphHandle {
    if name == ":memory:" {
        return Arc::new(Mutex::new(GraphDb {
            name: name.to_string(),
            nodes: Vec::new(),
            edges: Vec::new(),
            next_id: 1,
            db: None,
        }));
    }

    let db_path = format!("{}.graph.db", name);
    // R4: fall back to an in-memory graph instead of aborting if the file can't be opened.
    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[VargOS] graph_open('{}') failed: {} — falling back to in-memory", db_path, e);
            return Arc::new(Mutex::new(GraphDb {
                name: name.to_string(), nodes: Vec::new(), edges: Vec::new(), next_id: 1, db: None,
            }));
        }
    };
    init_graph_schema(&conn);
    let (nodes, edges, max_id) = load_graph_from_db(&conn);

    Arc::new(Mutex::new(GraphDb {
        name: name.to_string(),
        nodes,
        edges,
        next_id: max_id + 1, // R1: per-instance counter starts above all loaded IDs
        db: Some(conn),
    }))
}

/// Add a node with a label and properties, returns node ID
pub fn __varg_graph_add_node(
    graph: &GraphHandle,
    label: &str,
    props: &HashMap<String, String>,
) -> i64 {
    // R1/R2: take the lock first, draw the ID from this graph's own counter, and recover from
    // a poisoned lock instead of cascading panics.
    let mut g = graph.lock().unwrap_or_else(|e| e.into_inner());
    let id = g.next_id;
    g.next_id += 1;
    let node = GraphNode {
        id,
        label: label.to_string(),
        properties: props.clone(),
    };

    // Write-through to SQLite if persisted
    if let Some(ref conn) = g.db {
        let props_json = serde_json::to_string(props).unwrap_or_else(|_| "{}".to_string());
        // B10: surface write-through failures instead of silently dropping them (`.ok()`),
        // which previously caused the in-memory graph and its SQLite backing to diverge
        // without any signal.
        if let Err(e) = conn.execute(
            "INSERT INTO graph_nodes (id, label, properties) VALUES (?1, ?2, ?3)",
            rusqlite::params![id as i64, label, props_json],
        ) {
            eprintln!("[VargOS] graph node write-through failed: {}", e);
        }
    }

    g.nodes.push(node);
    id as i64
}

/// Add a directed edge between two nodes
pub fn __varg_graph_add_edge(
    graph: &GraphHandle,
    from_id: i64,
    relation: &str,
    to_id: i64,
    props: &HashMap<String, String>,
) {
    let edge = GraphEdge {
        from: from_id as u64,
        to: to_id as u64,
        relation: relation.to_string(),
        properties: props.clone(),
    };

    let mut g = graph.lock().unwrap_or_else(|e| e.into_inner());

    // Write-through to SQLite if persisted
    if let Some(ref conn) = g.db {
        let props_json = serde_json::to_string(props).unwrap_or_else(|_| "{}".to_string());
        // B10: surface write-through failures instead of silently dropping them.
        if let Err(e) = conn.execute(
            "INSERT INTO graph_edges (from_id, to_id, relation, properties) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![from_id, to_id, relation, props_json],
        ) {
            eprintln!("[VargOS] graph edge write-through failed: {}", e);
        }
    }

    g.edges.push(edge);
}

/// Query nodes by label, returns list of node property maps
pub fn __varg_graph_query(
    graph: &GraphHandle,
    label: &str,
) -> Vec<HashMap<String, String>> {
    let g = graph.lock().unwrap_or_else(|e| e.into_inner());
    g.nodes
        .iter()
        .filter(|n| n.label == label)
        .map(|n| {
            let mut props = n.properties.clone();
            props.insert("_id".to_string(), n.id.to_string());
            props.insert("_label".to_string(), n.label.clone());
            props
        })
        .collect()
}

/// Traverse from a node up to `depth` hops, optionally filtering by relation
pub fn __varg_graph_traverse(
    graph: &GraphHandle,
    start_id: i64,
    depth: i64,
    relation_filter: &str,
) -> Vec<HashMap<String, String>> {
    if depth <= 0 {
        return Vec::new();
    }
    let g = graph.lock().unwrap_or_else(|e| e.into_inner());
    let mut visited = std::collections::HashSet::new();
    let mut results = Vec::new();
    let mut frontier = vec![start_id as u64];

    for _ in 0..depth {
        let mut next_frontier = Vec::new();
        for &node_id in &frontier {
            if !visited.insert(node_id) {
                continue;
            }
            for edge in &g.edges {
                if edge.from == node_id {
                    if relation_filter.is_empty()
                        || relation_filter.split('|').any(|r| r.trim() == edge.relation)
                    {
                        next_frontier.push(edge.to);
                    }
                }
            }
        }
        frontier = next_frontier;
    }

    // Collect all reached nodes
    for &node_id in &frontier {
        if let Some(node) = g.nodes.iter().find(|n| n.id == node_id) {
            let mut props = node.properties.clone();
            props.insert("_id".to_string(), node.id.to_string());
            props.insert("_label".to_string(), node.label.clone());
            results.push(props);
        }
    }

    results
}

/// Get all neighbors of a node (both directions)
pub fn __varg_graph_neighbors(
    graph: &GraphHandle,
    node_id: i64,
) -> Vec<HashMap<String, String>> {
    let g = graph.lock().unwrap_or_else(|e| e.into_inner());
    let nid = node_id as u64;
    let mut results = Vec::new();

    for edge in &g.edges {
        let target = if edge.from == nid {
            Some(edge.to)
        } else if edge.to == nid {
            Some(edge.from)
        } else {
            None
        };
        if let Some(tid) = target {
            if let Some(node) = g.nodes.iter().find(|n| n.id == tid) {
                let mut props = node.properties.clone();
                props.insert("_id".to_string(), node.id.to_string());
                props.insert("_label".to_string(), node.label.clone());
                props.insert("_relation".to_string(), edge.relation.clone());
                results.push(props);
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_open_memory() {
        let g = __varg_graph_open(":memory:");
        let db = g.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(db.name, ":memory:");
        assert!(db.nodes.is_empty());
        assert!(db.db.is_none());
    }

    #[test]
    fn test_r4_graph_open_corrupt_db_does_not_panic() {
        // R4 regression: a file that is not a valid SQLite database used to abort the process
        // (unwrap/expect on schema init and every load query). graph_open must instead survive
        // and remain usable in-memory.
        let mut base = std::env::temp_dir();
        // Unique-ish name without Date/rand (unavailable): use thread id + a fixed tag.
        base.push(format!("varg_r4_corrupt_{:?}", std::thread::current().id()));
        let corrupt_path = format!("{}.graph.db", base.to_string_lossy());
        std::fs::write(&corrupt_path, b"this is definitely not a sqlite database file")
            .expect("write corrupt file");

        // Must not panic despite the corrupt backing file.
        let g = __varg_graph_open(&base.to_string_lossy());
        // In-memory operations still work.
        let id = __varg_graph_add_node(&g, "X", &HashMap::new());
        assert_eq!(id, 1, "first node id should be 1 despite corrupt DB");
        {
            let db = g.lock().unwrap_or_else(|e| e.into_inner());
            assert_eq!(db.nodes.len(), 1);
        }
        let _ = std::fs::remove_file(&corrupt_path);
    }

    #[test]
    fn test_graph_add_node_memory() {
        let g = __varg_graph_open(":memory:");
        let props = HashMap::from([("name".to_string(), "Alice".to_string())]);
        let id = __varg_graph_add_node(&g, "Person", &props);
        assert!(id > 0);
        let db = g.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(db.nodes.len(), 1);
        assert_eq!(db.nodes[0].label, "Person");
    }

    #[test]
    fn test_graph_add_edge_memory() {
        let g = __varg_graph_open(":memory:");
        let p1 = HashMap::from([("name".to_string(), "Alice".to_string())]);
        let p2 = HashMap::from([("name".to_string(), "Varg".to_string())]);
        let id1 = __varg_graph_add_node(&g, "Person", &p1);
        let id2 = __varg_graph_add_node(&g, "Project", &p2);
        __varg_graph_add_edge(&g, id1, "works_on", id2, &HashMap::new());
        let db = g.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(db.edges.len(), 1);
        assert_eq!(db.edges[0].relation, "works_on");
    }

    #[test]
    fn test_graph_query_by_label() {
        let g = __varg_graph_open(":memory:");
        let p1 = HashMap::from([("name".to_string(), "Alice".to_string())]);
        let p2 = HashMap::from([("name".to_string(), "Bob".to_string())]);
        let p3 = HashMap::from([("name".to_string(), "Varg".to_string())]);
        __varg_graph_add_node(&g, "Person", &p1);
        __varg_graph_add_node(&g, "Person", &p2);
        __varg_graph_add_node(&g, "Project", &p3);

        let persons = __varg_graph_query(&g, "Person");
        assert_eq!(persons.len(), 2);
        let projects = __varg_graph_query(&g, "Project");
        assert_eq!(projects.len(), 1);
    }

    #[test]
    fn test_graph_traverse() {
        let g = __varg_graph_open(":memory:");
        let p1 = HashMap::from([("name".to_string(), "Alice".to_string())]);
        let p2 = HashMap::from([("name".to_string(), "Bob".to_string())]);
        let p3 = HashMap::from([("name".to_string(), "Charlie".to_string())]);
        let id1 = __varg_graph_add_node(&g, "Person", &p1);
        let id2 = __varg_graph_add_node(&g, "Person", &p2);
        let id3 = __varg_graph_add_node(&g, "Person", &p3);
        __varg_graph_add_edge(&g, id1, "knows", id2, &HashMap::new());
        __varg_graph_add_edge(&g, id2, "knows", id3, &HashMap::new());

        // Depth 1: Alice -> Bob
        let result = __varg_graph_traverse(&g, id1, 1, "knows");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get("name").unwrap(), "Bob");

        // Depth 2: Alice -> Bob -> Charlie
        let result = __varg_graph_traverse(&g, id1, 2, "knows");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get("name").unwrap(), "Charlie");
    }

    #[test]
    fn test_graph_neighbors() {
        let g = __varg_graph_open(":memory:");
        let p1 = HashMap::from([("name".to_string(), "Alice".to_string())]);
        let p2 = HashMap::from([("name".to_string(), "Bob".to_string())]);
        let id1 = __varg_graph_add_node(&g, "Person", &p1);
        let id2 = __varg_graph_add_node(&g, "Person", &p2);
        __varg_graph_add_edge(&g, id1, "knows", id2, &HashMap::new());

        let n = __varg_graph_neighbors(&g, id1);
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].get("name").unwrap(), "Bob");
    }

    // ── Adversarial / edge-case tests ────────────────────────────────────────

    #[test]
    fn test_graph_query_nonexistent_label_returns_empty() {
        let g = __varg_graph_open(":memory:");
        __varg_graph_add_node(&g, "Person", &HashMap::from([("name".to_string(), "Alice".to_string())]));
        let results = __varg_graph_query(&g, "Robot");
        assert!(results.is_empty(), "querying unknown label must return empty vec");
    }

    #[test]
    fn test_graph_neighbors_isolated_node_returns_empty() {
        let g = __varg_graph_open(":memory:");
        let id = __varg_graph_add_node(&g, "Lone", &HashMap::new());
        let n = __varg_graph_neighbors(&g, id);
        assert!(n.is_empty(), "isolated node must have no neighbors");
    }

    #[test]
    fn test_graph_neighbors_both_directions() {
        // Alice→Bob: neighbors(Bob) should return Alice (reverse lookup)
        let g = __varg_graph_open(":memory:");
        let a = __varg_graph_add_node(&g, "P", &HashMap::from([("name".to_string(), "Alice".to_string())]));
        let b = __varg_graph_add_node(&g, "P", &HashMap::from([("name".to_string(), "Bob".to_string())]));
        __varg_graph_add_edge(&g, a, "knows", b, &HashMap::new());

        let nb = __varg_graph_neighbors(&g, b);
        assert_eq!(nb.len(), 1, "Bob must see Alice as neighbor via reverse edge");
        assert_eq!(nb[0].get("name").unwrap(), "Alice");
    }

    #[test]
    fn test_graph_traverse_zero_depth_returns_empty() {
        // depth=0 means "don't traverse" → must return empty, not the start node
        let g = __varg_graph_open(":memory:");
        let a = __varg_graph_add_node(&g, "P", &HashMap::from([("name".to_string(), "Alice".to_string())]));
        let b = __varg_graph_add_node(&g, "P", &HashMap::from([("name".to_string(), "Bob".to_string())]));
        __varg_graph_add_edge(&g, a, "knows", b, &HashMap::new());

        let result = __varg_graph_traverse(&g, a, 0, "knows");
        assert!(result.is_empty(), "depth=0 must return empty, not the start node");
    }

    #[test]
    fn test_graph_traverse_no_matching_relation_returns_empty() {
        let g = __varg_graph_open(":memory:");
        let a = __varg_graph_add_node(&g, "P", &HashMap::new());
        let b = __varg_graph_add_node(&g, "P", &HashMap::new());
        __varg_graph_add_edge(&g, a, "knows", b, &HashMap::new());

        let result = __varg_graph_traverse(&g, a, 1, "hates");
        assert!(result.is_empty(), "non-matching relation filter must yield no results");
    }

    #[test]
    fn test_graph_traverse_cycle_terminates() {
        // A→B, B→A (cycle). depth=10 must terminate, not loop forever.
        let g = __varg_graph_open(":memory:");
        let a = __varg_graph_add_node(&g, "P", &HashMap::new());
        let b = __varg_graph_add_node(&g, "P", &HashMap::new());
        __varg_graph_add_edge(&g, a, "to", b, &HashMap::new());
        __varg_graph_add_edge(&g, b, "to", a, &HashMap::new());

        // Should terminate; visited set blocks re-entering already-visited nodes
        let result = __varg_graph_traverse(&g, a, 10, "to");
        // After depth 3 the frontier is empty (A visited on iter 1, B on iter 2, A skipped on iter 3 → empty frontier)
        assert!(result.is_empty(), "cycle must terminate with empty frontier after visited set kicks in");
    }

    #[test]
    fn test_graph_add_edge_with_nonexistent_nodes_is_accepted() {
        // No validation — edges can reference node IDs that do not exist
        let g = __varg_graph_open(":memory:");
        __varg_graph_add_edge(&g, 9999, "phantom", 8888, &HashMap::new());
        let db = g.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(db.edges.len(), 1, "edge with nonexistent node IDs must still be stored");
        // neighbors and traverse for these IDs just return empty (find returns None)
        drop(db);
        let n = __varg_graph_neighbors(&g, 9999);
        assert!(n.is_empty(), "neighbors of ghost node must be empty (node record not found)");
    }

    #[test]
    fn test_graph_query_includes_meta_fields() {
        // query results must contain _id and _label injected by __varg_graph_query
        let g = __varg_graph_open(":memory:");
        __varg_graph_add_node(&g, "Thing", &HashMap::from([("x".to_string(), "1".to_string())]));
        let results = __varg_graph_query(&g, "Thing");
        assert_eq!(results.len(), 1);
        assert!(results[0].contains_key("_id"), "query result must contain _id");
        assert_eq!(results[0].get("_label").unwrap(), "Thing");
    }

    #[test]
    fn test_graph_multiple_nodes_same_label_all_returned() {
        let g = __varg_graph_open(":memory:");
        for i in 0..5 {
            __varg_graph_add_node(&g, "Item", &HashMap::from([("i".to_string(), i.to_string())]));
        }
        __varg_graph_add_node(&g, "Other", &HashMap::new());
        let results = __varg_graph_query(&g, "Item");
        assert_eq!(results.len(), 5, "all nodes with matching label must be returned");
    }

    #[test]
    fn test_graph_persistence_roundtrip() {
        let db_name = format!("test_graph_persist_{}", std::process::id());
        let db_path = format!("{}.graph.db", db_name);

        // Clean up from any previous run
        std::fs::remove_file(&db_path).ok();

        // Create graph and add data
        {
            let g = __varg_graph_open(&db_name);
            let p1 = HashMap::from([("name".to_string(), "Alice".to_string())]);
            let p2 = HashMap::from([("name".to_string(), "Bob".to_string())]);
            let id1 = __varg_graph_add_node(&g, "Person", &p1);
            let id2 = __varg_graph_add_node(&g, "Person", &p2);
            __varg_graph_add_edge(&g, id1, "knows", id2, &HashMap::new());
        }
        // GraphDb dropped here, but SQLite has the data

        // Reopen and verify data persisted
        {
            let g = __varg_graph_open(&db_name);
            let db = g.lock().unwrap_or_else(|e| e.into_inner());
            assert_eq!(db.nodes.len(), 2);
            assert_eq!(db.edges.len(), 1);
            assert_eq!(db.nodes[0].label, "Person");
            assert_eq!(db.edges[0].relation, "knows");
        }

        // Clean up
        std::fs::remove_file(&db_path).ok();
    }
}
