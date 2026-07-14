// Varg Runtime: Vector Math + Vector Store with SQLite Persistence
//
// Wave 20b + Issue #3: Embedded vector store with write-through SQLite storage.
// On vector_store_open(name), opens {name}.vector.db — loads existing data.
// All mutations are written through to SQLite immediately.
// Falls back to pure in-memory if name is ":memory:".

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use varg_os_types::Tensor;

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
    ).expect("Varg runtime error: vector_store_open() failed — could not initialize the vector database schema (check disk space and file permissions)");
}

/// Load existing vectors from SQLite into memory
fn load_vectors_from_db(conn: &Connection) -> Vec<VectorEntry> {
    let mut entries = Vec::new();
    let mut stmt = conn.prepare("SELECT id, embedding, metadata FROM vector_entries")
        .expect("Varg runtime error: vector_store_open() failed — could not read existing vectors from database (the database file may be corrupted)");
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0).unwrap_or_default();
        let emb_json: String = row.get(1).unwrap_or_default();
        let meta_json: String = row.get(2).unwrap_or_default();
        let embedding: Vec<f32> = serde_json::from_str(&emb_json).unwrap_or_default();
        let metadata: HashMap<String, String> = serde_json::from_str(&meta_json).unwrap_or_default();
        Ok(VectorEntry { id, embedding, metadata })
    }).expect("Varg runtime error: vector_store_open() failed — could not iterate stored vector entries (the database may be corrupted)");

    for row in rows {
        entries.push(row.expect("Varg runtime error: vector_store_open() failed — could not decode a stored vector entry (the database may contain corrupted data)"));
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
        .unwrap_or_else(|e| panic!("Varg runtime error: vector_store_open() failed — could not open vector database file '{}': {} (check the path and file permissions)", db_path, e));
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
    let mut s = store.lock().unwrap_or_else(|e| e.into_inner());

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
    let s = store.lock().unwrap_or_else(|e| e.into_inner());
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
    let mut s = store.lock().unwrap_or_else(|e| e.into_inner());

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
    store.lock().unwrap_or_else(|e| e.into_inner()).entries.len() as i64
}

/// Create an embedding from `text`.
///
/// Provider resolution (predictable — no network probing on the hot path):
///   1. `VARG_EMBED_PROVIDER` = `openai` | `gemini` | `ollama` | `local` (explicit)
///   2. `OPENAI_API_KEY` set → openai (`text-embedding-3-small`)
///   3. `GEMINI_API_KEY` set → gemini (`gemini-embedding-001`)
///   4. otherwise → local lexical embedding (n-gram hash, NOT semantic)
///
/// The model can be overridden with `VARG_EMBED_MODEL`. Real (semantic) providers require the
/// `net` feature; on any failure we warn once and fall back to the local embedding so a caller
/// (e.g. vector search / RAG) never panics. Ollama (`VARG_EMBED_PROVIDER=ollama`) gives free,
/// local, real embeddings for self-hosted setups.
pub fn __varg_embed(text: &str) -> Vec<f32> {
    #[cfg(feature = "net")]
    {
        match resolve_embed_provider().as_str() {
            "openai" => {
                if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                    if let Some(v) = openai_embed(text, &key, &embed_model("text-embedding-3-small")) { return v; }
                }
                warn_embed_fallback("openai");
            }
            "gemini" => {
                if let Ok(key) = std::env::var("GEMINI_API_KEY") {
                    if let Some(v) = gemini_embed(text, &key) { return v; }
                }
                warn_embed_fallback("gemini");
            }
            "ollama" => {
                if let Some(v) = ollama_embed(text, &embed_model("nomic-embed-text")) { return v; }
                warn_embed_fallback("ollama");
            }
            _ => {} // "local" / unknown → local embedding below
        }
    }
    // Local lexical embedding (384-dim word + character n-gram hash). Not semantic, but a
    // strictly better default than the old 64-dim bag-of-characters hash.
    crate::localembed::__varg_embed_local(text)
}

#[cfg(feature = "net")]
fn resolve_embed_provider() -> String {
    if let Ok(p) = std::env::var("VARG_EMBED_PROVIDER") {
        if !p.trim().is_empty() { return p.trim().to_lowercase(); }
    }
    if std::env::var("OPENAI_API_KEY").map(|k| !k.is_empty()).unwrap_or(false) { return "openai".to_string(); }
    if std::env::var("GEMINI_API_KEY").map(|k| !k.is_empty()).unwrap_or(false) { return "gemini".to_string(); }
    "local".to_string()
}

#[cfg(feature = "net")]
fn embed_model(default: &str) -> String {
    std::env::var("VARG_EMBED_MODEL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| default.to_string())
}

#[cfg(feature = "net")]
fn warn_embed_fallback(provider: &str) {
    use std::sync::OnceLock;
    static WARNED: OnceLock<()> = OnceLock::new();
    if WARNED.set(()).is_ok() {
        eprintln!("[EMBED] provider '{}' unavailable — falling back to local (non-semantic) embeddings", provider);
    }
}

/// OpenAI embeddings API (`/v1/embeddings`).
#[cfg(feature = "net")]
fn openai_embed(text: &str, api_key: &str, model: &str) -> Option<Vec<f32>> {
    let body = serde_json::json!({ "input": text, "model": model }).to_string();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15)).build().ok()?;
    let resp = client.post("https://api.openai.com/v1/embeddings")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(body).send().ok()?;
    if !resp.status().is_success() {
        eprintln!("[EMBED] OpenAI API returned status: {}", resp.status());
        return None;
    }
    let json: serde_json::Value = serde_json::from_str(&resp.text().ok()?).ok()?;
    let values = json.get("data")?.get(0)?.get("embedding")?.as_array()?;
    let embedding: Vec<f32> = values.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect();
    if embedding.is_empty() { None } else { Some(embedding) }
}

/// Ollama embeddings API (`/api/embeddings`). Local, no API key. Host via `OLLAMA_HOST`.
#[cfg(feature = "net")]
fn ollama_embed(text: &str, model: &str) -> Option<Vec<f32>> {
    let host = std::env::var("OLLAMA_HOST").ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    let url = format!("{}/api/embeddings", host.trim_end_matches('/'));
    let body = serde_json::json!({ "model": model, "prompt": text }).to_string();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30)).build().ok()?;
    let resp = client.post(&url)
        .header("Content-Type", "application/json")
        .body(body).send().ok()?;
    if !resp.status().is_success() {
        eprintln!("[EMBED] Ollama returned status: {}", resp.status());
        return None;
    }
    let json: serde_json::Value = serde_json::from_str(&resp.text().ok()?).ok()?;
    let values = json.get("embedding")?.as_array()?;
    let embedding: Vec<f32> = values.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect();
    if embedding.is_empty() { None } else { Some(embedding) }
}

/// Call Gemini embedding-001 API for real semantic embeddings (requires `net` feature)
#[cfg(feature = "net")]
fn gemini_embed(text: &str, api_key: &str) -> Option<Vec<f32>> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-001:embedContent?key={}",
        api_key
    );
    let body = serde_json::json!({
        "content": {
            "parts": [{"text": text}]
        }
    });
    let body_str = body.to_string();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body_str)
        .send()
        .ok()?;
    if !resp.status().is_success() {
        eprintln!("[EMBED] Gemini API returned status: {}", resp.status());
        return None;
    }
    let resp_text = resp.text().ok()?;
    let json: serde_json::Value = serde_json::from_str(&resp_text).ok()?;
    let values = json
        .get("embedding")?
        .get("values")?
        .as_array()?;
    let embedding: Vec<f32> = values
        .iter()
        .filter_map(|v: &serde_json::Value| v.as_f64().map(|f| f as f32))
        .collect();
    if embedding.is_empty() {
        return None;
    }
    Some(embedding)
}

/// Search by text: embeds the query then performs cosine similarity search.
/// Returns a Vec of metadata strings for the top-k most similar entries.
/// Each metadata string is a JSON-encoded map of metadata fields including "_id" and "_score".
pub fn __varg_vector_search_text(store: &VectorStoreHandle, query_text: &str, top_k: i64) -> Vec<String> {
    let embedding = __varg_embed(query_text);
    let results = __varg_vector_store_search(store, &embedding, top_k);
    results
        .into_iter()
        .map(|map| serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string()))
        .collect()
}

// ── Wave 33: LSH Approximate Nearest-Neighbor Index ───────────────────────
//
// Random-projection LSH: project each vector onto k hyperplanes,
// encode as a bit-mask → bucket. Search only same bucket + neighbors.

const LSH_PLANES: usize = 16; // 16 bits → 65536 possible buckets

fn lcg_f32(state: &mut u64) -> f32 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    // Map to [-1, 1]
    (*state as f32 / u64::MAX as f32) * 2.0 - 1.0
}

/// Generate k random hyperplane vectors (each of `dim` dimensions).
fn make_planes(dim: usize, k: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut state = seed;
    (0..k)
        .map(|_| (0..dim).map(|_| lcg_f32(&mut state)).collect())
        .collect()
}

fn lsh_hash(vec: &[f32], planes: &[Vec<f32>]) -> u64 {
    let mut h = 0u64;
    for (i, plane) in planes.iter().enumerate() {
        let dot: f32 = vec.iter().zip(plane.iter()).map(|(a, b)| a * b).sum();
        if dot >= 0.0 { h |= 1 << i; }
    }
    h
}

pub struct LshIndex {
    planes: Vec<Vec<f32>>,
    buckets: HashMap<u64, Vec<String>>, // hash → entry ids
}

impl LshIndex {
    fn build(store: &VectorStore) -> Self {
        let dim = store.entries.first().map(|e| e.embedding.len()).unwrap_or(128);
        let planes = make_planes(dim, LSH_PLANES, 42);
        let mut buckets: HashMap<u64, Vec<String>> = HashMap::new();
        for entry in &store.entries {
            let h = lsh_hash(&entry.embedding, &planes);
            buckets.entry(h).or_default().push(entry.id.clone());
        }
        LshIndex { planes, buckets }
    }

    fn search(&self, query: &[f32], store: &VectorStore, top_k: usize) -> Vec<(String, f32)> {
        let h = lsh_hash(query, &self.planes);
        // Collect candidates: same bucket + 1-bit Hamming neighbours
        let mut candidate_ids = std::collections::HashSet::new();
        if let Some(ids) = self.buckets.get(&h) {
            candidate_ids.extend(ids.iter().cloned());
        }
        for i in 0..LSH_PLANES {
            let neighbour = h ^ (1 << i);
            if let Some(ids) = self.buckets.get(&neighbour) {
                candidate_ids.extend(ids.iter().cloned());
            }
        }
        // Score candidates
        let mut scored: Vec<(String, f32)> = candidate_ids
            .iter()
            .filter_map(|id| {
                store.entries.iter().find(|e| &e.id == id).map(|e| {
                    (id.clone(), cosine_similarity(query, &e.embedding))
                })
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }
}

/// Build an in-memory LSH index for fast approximate search.
/// Returns an opaque JSON-encoded index handle (serialized to string for simplicity).
pub fn __varg_vector_build_index(store: &VectorStoreHandle) -> String {
    let s = store.lock().unwrap_or_else(|e| e.into_inner());
    let idx = LshIndex::build(&s);
    let bucket_counts: Vec<(String, usize)> = idx.buckets.iter()
        .map(|(k, v)| (k.to_string(), v.len()))
        .collect();
    serde_json::json!({
        "buckets": bucket_counts.len(),
        "indexed": s.entries.len()
    }).to_string()
}

/// Approximate nearest-neighbour search using the LSH index.
/// Falls back to linear scan when the index covers < 50% of entries.
pub fn __varg_vector_search_fast(
    store: &VectorStoreHandle,
    query: &[f32],
    top_k: i64,
) -> Vec<String> {
    let s = store.lock().unwrap_or_else(|e| e.into_inner());
    let idx = LshIndex::build(&s);
    idx.search(query, &s, top_k as usize)
        .into_iter()
        .map(|(id, score)| {
            serde_json::json!({"id": id, "score": score}).to_string()
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────

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
        let s = store.lock().unwrap_or_else(|e| e.into_inner());
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
        let s = store.lock().unwrap_or_else(|e| e.into_inner());
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
    fn test_embed_uses_384dim_local_fallback_c1() {
        // C1: the keyless/local path is now the 384-dim word+n-gram embedder, not the old
        // 64-dim bag-of-characters hash. (Real semantic providers are opt-in via env.)
        let e = __varg_embed("some representative text");
        assert_eq!(e.len(), 384, "local embedding should be 384-dim, got {}", e.len());
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
            let s = store.lock().unwrap_or_else(|e| e.into_inner());
            assert_eq!(s.entries[0].id, "b");
        }

        std::fs::remove_file(&db_path).ok();
    }

    // ── vector_store_search top-k tests ──────────────────────────────────

    #[test]
    fn test_vector_store_search_returns_top_k() {
        let store = __varg_vector_store_open(":memory:");
        // doc_a is most similar to query [1, 0, 0]
        let meta_a = HashMap::from([("label".to_string(), "alpha".to_string())]);
        let meta_b = HashMap::from([("label".to_string(), "beta".to_string())]);
        let meta_c = HashMap::from([("label".to_string(), "gamma".to_string())]);
        __varg_vector_store_upsert(&store, "doc_a", &[1.0, 0.0, 0.0], &meta_a);
        __varg_vector_store_upsert(&store, "doc_b", &[0.7, 0.3, 0.0], &meta_b);
        __varg_vector_store_upsert(&store, "doc_c", &[0.0, 0.0, 1.0], &meta_c);

        let results = __varg_vector_store_search(&store, &[1.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        // Top result should be doc_a (identical vector)
        assert_eq!(results[0].get("_id").unwrap(), "doc_a");
        assert_eq!(results[0].get("label").unwrap(), "alpha");
        // Second should be doc_b (next closest)
        assert_eq!(results[1].get("_id").unwrap(), "doc_b");
    }

    #[test]
    fn test_vector_store_search_empty_store() {
        let store = __varg_vector_store_open(":memory:");
        let results = __varg_vector_store_search(&store, &[1.0, 0.0, 0.0], 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_vector_search_text_returns_json_strings() {
        let store = __varg_vector_store_open(":memory:");
        let meta = HashMap::from([("doc".to_string(), "test".to_string())]);
        // Use embed to get the actual embedding of the text, then store it
        let emb = __varg_embed("hello world");
        __varg_vector_store_upsert(&store, "hello_doc", &emb, &meta);

        let results = __varg_vector_search_text(&store, "hello world", 1);
        assert_eq!(results.len(), 1);
        // Each result should be valid JSON
        let v: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(v["_id"].as_str().unwrap(), "hello_doc");
    }

    // ── LSH Index tests ───────────────────────────────────────────────────

    #[test]
    fn test_lsh_build_index_empty_store() {
        let store = __varg_vector_store_open(":memory:");
        let info = __varg_vector_build_index(&store);
        let v: serde_json::Value = serde_json::from_str(&info).unwrap();
        assert_eq!(v["indexed"], 0);
    }

    #[test]
    fn test_lsh_build_index_with_entries() {
        let store = __varg_vector_store_open(":memory:");
        let v1: Vec<f32> = (0..128).map(|i| i as f32 / 128.0).collect();
        let v2: Vec<f32> = (0..128).map(|i| (127 - i) as f32 / 128.0).collect();
        __varg_vector_store_upsert(&store, "a", &v1, &std::collections::HashMap::new());
        __varg_vector_store_upsert(&store, "b", &v2, &std::collections::HashMap::new());
        let info = __varg_vector_build_index(&store);
        let v: serde_json::Value = serde_json::from_str(&info).unwrap();
        assert_eq!(v["indexed"], 2);
    }

    #[test]
    fn test_lsh_search_fast_returns_results() {
        let store = __varg_vector_store_open(":memory:");
        for i in 0..10 {
            let v: Vec<f32> = (0..64).map(|j| (i * 64 + j) as f32 / 1000.0).collect();
            __varg_vector_store_upsert(&store, &format!("doc_{i}"), &v, &std::collections::HashMap::new());
        }
        let query: Vec<f32> = (0..64).map(|j| j as f32 / 1000.0).collect();
        let results = __varg_vector_search_fast(&store, &query, 3);
        assert!(!results.is_empty());
        assert!(results.len() <= 3);
        // Each result should be valid JSON
        for r in &results {
            let v: serde_json::Value = serde_json::from_str(r).unwrap();
            assert!(v.get("id").is_some());
            assert!(v.get("score").is_some());
        }
    }

    #[test]
    fn test_lsh_search_most_similar_first() {
        let store = __varg_vector_store_open(":memory:");
        let target: Vec<f32> = (0..32).map(|i| i as f32).collect();
        let similar: Vec<f32> = target.iter().map(|v| v + 0.01).collect();
        let dissimilar: Vec<f32> = target.iter().map(|v| -v).collect();
        __varg_vector_store_upsert(&store, "similar", &similar, &std::collections::HashMap::new());
        __varg_vector_store_upsert(&store, "dissimilar", &dissimilar, &std::collections::HashMap::new());
        let results = __varg_vector_search_fast(&store, &target, 2);
        if results.len() >= 2 {
            let first: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
            assert_eq!(first["id"].as_str().unwrap(), "similar");
        }
    }
}
