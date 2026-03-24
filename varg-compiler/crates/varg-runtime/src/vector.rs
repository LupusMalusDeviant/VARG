// Varg Runtime: Vector Math + Vector Store with SQLite Persistence
//
// Wave 20b + Issue #3: Embedded vector store with write-through SQLite storage.
// On vector_store_open(name), opens {name}.vector.db — loads existing data.
// All mutations are written through to SQLite immediately.
// Falls back to pure in-memory if name is ":memory:".

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use varg_os_types::{Embedding, Tensor};

/// Compute cosine similarity between two tensors
pub fn __varg_cosine_sim(a: &Tensor, b: &Tensor) -> f32 {
    let dot: f32 = a.data.iter().zip(b.data.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.data.iter().map(|v| v * v).sum::<f32>().sqrt();
    let mag_b: f32 = b.data.iter().map(|v| v * v).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { 0.0 } else { dot / (mag_a * mag_b) }
}

/// Create a Tensor from integer data
pub fn __varg_create_tensor(data: Vec<i64>) -> Tensor {
    let data_f32 = data.into_iter().map(|x| x as f32).collect::<Vec<f32>>();
    let len = data_f32.len();
    Tensor { data: data_f32, shape: vec![len] }
}

// ===== Vector Store =====

/// Cosine similarity between two float vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|v| v * v).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|v| v * v).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { 0.0 } else { dot / (mag_a * mag_b) }
}

#[derive(Debug, Clone)]
pub struct VectorEntry {
    pub id: String,
    pub embedding: Vec<f32>,
    pub metadata: HashMap<String, String>,
}

pub struct VectorStore {
    pub name: String,
    pub entries: Vec<VectorEntry>,
    /// SQLite connection for persistence (None = pure in-memory)
    db: Option<Connection>,
}

// Manual Debug since Connection doesn't implement Debug
impl std::fmt::Debug for VectorStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorStore")
            .field("name", &self.name)
            .field("entries_count", &self.entries.len())
            .field("persisted", &self.db.is_some())
            .finish()
    }
}

/// Shared, thread-safe vector store handle
pub type VectorStoreHandle = Arc<Mutex<VectorStore>>;

/// Initialize SQLite schema for vector persistence
fn init_vector_schema(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS vector_entries (
            id TEXT PRIMARY KEY,
            embedding TEXT NOT NULL,
            metadata TEXT NOT NULL
        );"
    ).expect("Failed to initialize vector schema");
}

/// Load existing vectors from SQLite into memory
fn load_vectors_from_db(conn: &Connection) -> Vec<VectorEntry> {
    let mut entries = Vec::new();
    let mut stmt = conn.prepare("SELECT id, embedding, metadata FROM vector_entries").unwrap();
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0).unwrap();
        let emb_json: String = row.get(1).unwrap();
        let meta_json: String = row.get(2).unwrap();
        let embedding: Vec<f32> = serde_json::from_str(&emb_json).unwrap_or_default();
        let metadata: HashMap<String, String> = serde_json::from_str(&meta_json).unwrap_or_default();
        Ok(VectorEntry { id, embedding, metadata })
    }).unwrap();

    for row in rows {
        entries.push(row.unwrap());
    }
    entries
}

/// Open or create a named vector store
/// If name is ":memory:", uses pure in-memory mode.
/// Otherwise, persists to {name}.vector.db
pub fn __varg_vector_store_open(name: &str) -> VectorStoreHandle {
    if name == ":memory:" {
        return Arc::new(Mutex::new(VectorStore {
            name: name.to_string(),
            entries: Vec::new(),
            db: None,
        }));
    }

    let db_path = format!("{}.vector.db", name);
    let conn = Connection::open(&db_path)
        .unwrap_or_else(|e| panic!("Failed to open vector database '{}': {}", db_path, e));
    init_vector_schema(&conn);
    let entries = load_vectors_from_db(&conn);

    Arc::new(Mutex::new(VectorStore {
        name: name.to_string(),
        entries,
        db: Some(conn),
    }))
}

/// Upsert a vector with ID, embedding, and metadata
pub fn __varg_vector_store_upsert(
    store: &VectorStoreHandle,
    id: &str,
    embedding: &[f32],
    metadata: &HashMap<String, String>,
) {
    let mut s = store.lock().unwrap();

    // Write-through to SQLite if persisted
    if let Some(ref conn) = s.db {
        let emb_json = serde_json::to_string(&embedding.to_vec()).unwrap_or_else(|_| "[]".to_string());
        let meta_json = serde_json::to_string(metadata).unwrap_or_else(|_| "{}".to_string());
        conn.execute(
            "INSERT OR REPLACE INTO vector_entries (id, embedding, metadata) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, emb_json, meta_json],
        ).ok();
    }

    // Update in-memory
    if let Some(entry) = s.entries.iter_mut().find(|e| e.id == id) {
        entry.embedding = embedding.to_vec();
        entry.metadata = metadata.clone();
    } else {
        s.entries.push(VectorEntry {
            id: id.to_string(),
            embedding: embedding.to_vec(),
            metadata: metadata.clone(),
        });
    }
}

/// Search for top_k nearest vectors by cosine similarity
/// Returns list of maps with _id, _score, and all metadata fields
pub fn __varg_vector_store_search(
    store: &VectorStoreHandle,
    query: &[f32],
    top_k: i64,
) -> Vec<HashMap<String, String>> {
    let s = store.lock().unwrap();
    let mut scored: Vec<(&VectorEntry, f32)> = s.entries
        .iter()
        .map(|e| (e, cosine_similarity(query, &e.embedding)))
        .collect();

    // Sort by similarity descending
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scored.into_iter()
        .take(top_k as usize)
        .map(|(entry, score)| {
            let mut result = entry.metadata.clone();
            result.insert("_id".to_string(), entry.id.clone());
            result.insert("_score".to_string(), format!("{:.4}", score));
            result
        })
        .collect()
}

/// Delete a vector by ID
pub fn __varg_vector_store_delete(store: &VectorStoreHandle, id: &str) -> bool {
    let mut s = store.lock().unwrap();

    // Write-through to SQLite if persisted
    if let Some(ref conn) = s.db {
        conn.execute("DELETE FROM vector_entries WHERE id = ?1", rusqlite::params![id]).ok();
    }

    let before = s.entries.len();
    s.entries.retain(|e| e.id != id);
    s.entries.len() < before
}

/// Count entries in the store
pub fn __varg_vector_store_count(store: &VectorStoreHandle) -> i64 {
    store.lock().unwrap().entries.len() as i64
}

/// Create an embedding from text using simple bag-of-characters hash
/// This is a LOCAL placeholder — real embed() would call an LLM provider
pub fn __varg_embed(text: &str) -> Vec<f32> {
    // Simple deterministic embedding: hash each character position
    // 64-dimensional embedding for demonstration purposes
    let dim = 64;
    let mut vec = vec![0.0f32; dim];
    for (i, ch) in text.chars().enumerate() {
        let idx = (ch as usize + i * 7) % dim;
        vec[idx] += 1.0;
    }
    // Normalize
    let mag: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if mag > 0.0 {
        for v in &mut vec {
            *v /= mag;
        }
    }
    vec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_sim_identical() {
        let a = Tensor { data: vec![1.0, 2.0, 3.0], shape: vec![3] };
        let b = Tensor { data: vec![1.0, 2.0, 3.0], shape: vec![3] };
        let sim = __varg_cosine_sim(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_sim_orthogonal() {
        let a = Tensor { data: vec![1.0, 0.0], shape: vec![2] };
        let b = Tensor { data: vec![0.0, 1.0], shape: vec![2] };
        let sim = __varg_cosine_sim(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_sim_zero_vector() {
        let a = Tensor { data: vec![0.0, 0.0], shape: vec![2] };
        let b = Tensor { data: vec![1.0, 1.0], shape: vec![2] };
        assert_eq!(__varg_cosine_sim(&a, &b), 0.0);
    }

    #[test]
    fn test_create_tensor() {
        let t = __varg_create_tensor(vec![1, 2, 3]);
        assert_eq!(t.data, vec![1.0, 2.0, 3.0]);
        assert_eq!(t.shape, vec![3]);
    }

    #[test]
    fn test_vector_store_open_memory() {
        let store = __varg_vector_store_open(":memory:");
        let s = store.lock().unwrap();
        assert_eq!(s.name, ":memory:");
        assert!(s.entries.is_empty());
        assert!(s.db.is_none());
    }

    #[test]
    fn test_vector_store_upsert_and_count() {
        let store = __varg_vector_store_open(":memory:");
        let meta = HashMap::from([("source".to_string(), "test".to_string())]);
        __varg_vector_store_upsert(&store, "doc1", &[1.0, 0.0, 0.0], &meta);
        __varg_vector_store_upsert(&store, "doc2", &[0.0, 1.0, 0.0], &meta);
        assert_eq!(__varg_vector_store_count(&store), 2);
    }

    #[test]
    fn test_vector_store_upsert_overwrites() {
        let store = __varg_vector_store_open(":memory:");
        let meta1 = HashMap::from([("v".to_string(), "1".to_string())]);
        let meta2 = HashMap::from([("v".to_string(), "2".to_string())]);
        __varg_vector_store_upsert(&store, "doc1", &[1.0, 0.0], &meta1);
        __varg_vector_store_upsert(&store, "doc1", &[0.0, 1.0], &meta2);
        assert_eq!(__varg_vector_store_count(&store), 1);
        let s = store.lock().unwrap();
        assert_eq!(s.entries[0].metadata.get("v").unwrap(), "2");
    }

    #[test]
    fn test_vector_store_search() {
        let store = __varg_vector_store_open(":memory:");
        let empty = HashMap::new();
        __varg_vector_store_upsert(&store, "a", &[1.0, 0.0, 0.0], &empty);
        __varg_vector_store_upsert(&store, "b", &[0.9, 0.1, 0.0], &empty);
        __varg_vector_store_upsert(&store, "c", &[0.0, 0.0, 1.0], &empty);

        let results = __varg_vector_store_search(&store, &[1.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].get("_id").unwrap(), "a");
        assert_eq!(results[1].get("_id").unwrap(), "b");
    }

    #[test]
    fn test_vector_store_delete() {
        let store = __varg_vector_store_open(":memory:");
        let empty = HashMap::new();
        __varg_vector_store_upsert(&store, "doc1", &[1.0], &empty);
        __varg_vector_store_upsert(&store, "doc2", &[2.0], &empty);
        assert_eq!(__varg_vector_store_count(&store), 2);

        let deleted = __varg_vector_store_delete(&store, "doc1");
        assert!(deleted);
        assert_eq!(__varg_vector_store_count(&store), 1);

        let not_deleted = __varg_vector_store_delete(&store, "nonexistent");
        assert!(!not_deleted);
    }

    #[test]
    fn test_embed_deterministic() {
        let e1 = __varg_embed("hello world");
        let e2 = __varg_embed("hello world");
        assert_eq!(e1, e2);
    }

    #[test]
    fn test_embed_similar_texts() {
        let e1 = __varg_embed("the cat sat on the mat");
        let e2 = __varg_embed("the cat sat on a mat");
        let e3 = __varg_embed("quantum physics equations");
        let sim_close = cosine_similarity(&e1, &e2);
        let sim_far = cosine_similarity(&e1, &e3);
        // Similar texts should have higher similarity
        assert!(sim_close > sim_far);
    }

    #[test]
    fn test_embed_normalized() {
        let e = __varg_embed("test text");
        let mag: f32 = e.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((mag - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_vector_persistence_roundtrip() {
        let store_name = format!("test_vec_persist_{}", std::process::id());
        let db_path = format!("{}.vector.db", store_name);

        // Clean up from any previous run
        std::fs::remove_file(&db_path).ok();

        // Create store and add data
        {
            let store = __varg_vector_store_open(&store_name);
            let meta = HashMap::from([("tag".to_string(), "test".to_string())]);
            __varg_vector_store_upsert(&store, "v1", &[1.0, 0.0, 0.0], &meta);
            __varg_vector_store_upsert(&store, "v2", &[0.0, 1.0, 0.0], &meta);
        }
        // Store dropped, SQLite has the data

        // Reopen and verify data persisted
        {
            let store = __varg_vector_store_open(&store_name);
            assert_eq!(__varg_vector_store_count(&store), 2);
            let results = __varg_vector_store_search(&store, &[1.0, 0.0, 0.0], 1);
            assert_eq!(results[0].get("_id").unwrap(), "v1");
            assert_eq!(results[0].get("tag").unwrap(), "test");
        }

        // Clean up
        std::fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_vector_persistence_delete() {
        let store_name = format!("test_vec_del_{}", std::process::id());
        let db_path = format!("{}.vector.db", store_name);
        std::fs::remove_file(&db_path).ok();

        {
            let store = __varg_vector_store_open(&store_name);
            let empty = HashMap::new();
            __varg_vector_store_upsert(&store, "a", &[1.0], &empty);
            __varg_vector_store_upsert(&store, "b", &[2.0], &empty);
            __varg_vector_store_delete(&store, "a");
        }

        // Reopen — should only have "b"
        {
            let store = __varg_vector_store_open(&store_name);
            assert_eq!(__varg_vector_store_count(&store), 1);
            let s = store.lock().unwrap();
            assert_eq!(s.entries[0].id, "b");
        }

        std::fs::remove_file(&db_path).ok();
    }
}
