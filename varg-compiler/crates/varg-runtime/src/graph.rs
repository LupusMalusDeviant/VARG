// Wave 20: Native Knowledge Graph Runtime
//
// Embedded graph engine using adjacency lists.
// No external dependencies — pure Rust implementation.
// Can be swapped for SurrealDB backend later.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NODE_COUNTER: AtomicU64 = AtomicU64::new(1);

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

#[derive(Debug, Clone)]
pub struct GraphDb {
    pub name: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Shared, thread-safe graph handle
pub type GraphHandle = Arc<Mutex<GraphDb>>;

/// Open or create a named graph database
pub fn __varg_graph_open(name: &str) -> GraphHandle {
    Arc::new(Mutex::new(GraphDb {
        name: name.to_string(),
        nodes: Vec::new(),
        edges: Vec::new(),
    }))
}

/// Add a node with a label and properties, returns node ID
pub fn __varg_graph_add_node(
    graph: &GraphHandle,
    label: &str,
    props: &HashMap<String, String>,
) -> i64 {
    let id = NODE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let node = GraphNode {
        id,
        label: label.to_string(),
        properties: props.clone(),
    };
    graph.lock().unwrap().nodes.push(node);
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
    graph.lock().unwrap().edges.push(edge);
}

/// Query nodes by label, returns list of node property maps
pub fn __varg_graph_query(
    graph: &GraphHandle,
    label: &str,
) -> Vec<HashMap<String, String>> {
    let g = graph.lock().unwrap();
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
    let g = graph.lock().unwrap();
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
    let g = graph.lock().unwrap();
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
    fn test_graph_open() {
        let g = __varg_graph_open("test_graph");
        let db = g.lock().unwrap();
        assert_eq!(db.name, "test_graph");
        assert!(db.nodes.is_empty());
    }

    #[test]
    fn test_graph_add_node() {
        let g = __varg_graph_open("test");
        let props = HashMap::from([("name".to_string(), "Alice".to_string())]);
        let id = __varg_graph_add_node(&g, "Person", &props);
        assert!(id > 0);
        let db = g.lock().unwrap();
        assert_eq!(db.nodes.len(), 1);
        assert_eq!(db.nodes[0].label, "Person");
    }

    #[test]
    fn test_graph_add_edge() {
        let g = __varg_graph_open("test");
        let p1 = HashMap::from([("name".to_string(), "Alice".to_string())]);
        let p2 = HashMap::from([("name".to_string(), "Varg".to_string())]);
        let id1 = __varg_graph_add_node(&g, "Person", &p1);
        let id2 = __varg_graph_add_node(&g, "Project", &p2);
        __varg_graph_add_edge(&g, id1, "works_on", id2, &HashMap::new());
        let db = g.lock().unwrap();
        assert_eq!(db.edges.len(), 1);
        assert_eq!(db.edges[0].relation, "works_on");
    }

    #[test]
    fn test_graph_query_by_label() {
        let g = __varg_graph_open("test");
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
        let g = __varg_graph_open("test");
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
        let g = __varg_graph_open("test");
        let p1 = HashMap::from([("name".to_string(), "Alice".to_string())]);
        let p2 = HashMap::from([("name".to_string(), "Bob".to_string())]);
        let id1 = __varg_graph_add_node(&g, "Person", &p1);
        let id2 = __varg_graph_add_node(&g, "Person", &p2);
        __varg_graph_add_edge(&g, id1, "knows", id2, &HashMap::new());

        let n = __varg_graph_neighbors(&g, id1);
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].get("name").unwrap(), "Bob");
    }
}
