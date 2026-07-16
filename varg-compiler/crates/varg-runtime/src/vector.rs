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
    /// Built HNSW index (feature `ann`). Held here so `vector_build_index` actually persists it —
    /// searches reuse it instead of rebuilding per query.
    #[cfg(feature = "ann")]
    ann: Option<ann_impl::AnnIndex>,
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
            #[cfg(feature = "ann")]
            ann: None,
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
        #[cfg(feature = "ann")]
        ann: None,
    }))
}

/// Accepts an embedding as either `f32` (what `embed()` returns) or `f64` (what a Varg float array
/// literal like `[1.0, 0.0]` compiles to). Without this, literal embeddings could not be passed to
/// the vector API at all — only `embed()` results.
pub trait ToF32Vec {
    fn to_f32_vec(&self) -> Vec<f32>;
}
impl ToF32Vec for Vec<f32> {
    fn to_f32_vec(&self) -> Vec<f32> { self.clone() }
}
impl ToF32Vec for [f32] {
    fn to_f32_vec(&self) -> Vec<f32> { self.to_vec() }
}
impl ToF32Vec for Vec<f64> {
    fn to_f32_vec(&self) -> Vec<f32> { self.iter().map(|v| *v as f32).collect() }
}
impl ToF32Vec for [f64] {
    fn to_f32_vec(&self) -> Vec<f32> { self.iter().map(|v| *v as f32).collect() }
}
// Fixed-size arrays (`&[1.0, 2.0, 3.0]`) don't coerce to a slice through a generic param.
impl<const N: usize> ToF32Vec for [f32; N] {
    fn to_f32_vec(&self) -> Vec<f32> { self.to_vec() }
}
impl<const N: usize> ToF32Vec for [f64; N] {
    fn to_f32_vec(&self) -> Vec<f32> { self.iter().map(|v| *v as f32).collect() }
}

/// Upsert a vector with ID, embedding, and metadata
pub fn __varg_vector_store_upsert<E: ToF32Vec + ?Sized>(
    store: &VectorStoreHandle,
    id: &str,
    embedding: &E,
    metadata: &HashMap<String, String>,
) {
    let embedding = embedding.to_f32_vec();
    let embedding: &[f32] = &embedding;
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
pub fn __varg_vector_store_search<Q: ToF32Vec + ?Sized>(
    store: &VectorStoreHandle,
    query: &Q,
    top_k: i64,
) -> Vec<HashMap<String, String>> {
    let query = query.to_f32_vec();
    let query: &[f32] = &query;
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

// ── Approximate Nearest-Neighbour Index (HNSW) ────────────────────────────
//
// Replaces the old random-projection LSH, which had two defects beyond weak recall:
// `vector_build_index` discarded the index it built, and `vector_search_fast` rebuilt the whole
// index on *every* query — making the "fast" path both approximate AND slower than a plain scan.
//
// Now: `vector_build_index` builds a real HNSW index and stores it on the handle; `search_fast`
// reuses it. Cosine ranking is preserved by L2-normalising vectors, where Euclidean order is
// equivalent to cosine order. Without the `ann` feature the store still answers `search_fast`
// exactly (brute force) — correct, just linear.

/// L2-normalise so Euclidean distance ranks identically to cosine similarity.
/// Only the HNSW index needs this; without `ann` it would be dead code (and a warning on every
/// default build of every Varg program).
#[cfg(feature = "ann")]
fn normalize(v: &[f32]) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 { return v.to_vec(); }
    v.iter().map(|x| x / norm).collect()
}

/// Exact top-k by cosine similarity — the fallback and the correctness baseline.
fn exact_top_k(store: &VectorStore, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
    let mut scored: Vec<(String, f32)> = store.entries.iter()
        .map(|e| (e.id.clone(), cosine_similarity(query, &e.embedding)))
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_k);
    scored
}

#[cfg(feature = "ann")]
mod ann_impl {
    use super::{normalize, VectorStore};
    use instant_distance::{Builder, HnswMap, Point, Search};

    #[derive(Clone, Debug)]
    pub struct VecPoint(pub Vec<f32>);

    impl Point for VecPoint {
        fn distance(&self, other: &Self) -> f32 {
            self.0.iter()
                .zip(other.0.iter())
                .map(|(a, b)| { let d = a - b; d * d })
                .sum::<f32>()
                .sqrt()
        }
    }

    pub struct AnnIndex {
        pub map: HnswMap<VecPoint, String>,
        /// Entry count the index was built from — used to detect staleness after upserts/deletes.
        pub indexed_count: usize,
    }

    pub fn build(store: &VectorStore) -> Option<AnnIndex> {
        if store.entries.is_empty() { return None; }
        let mut points = Vec::with_capacity(store.entries.len());
        let mut values = Vec::with_capacity(store.entries.len());
        for e in &store.entries {
            points.push(VecPoint(normalize(&e.embedding)));
            values.push(e.id.clone());
        }
        let map = Builder::default().build(points, values);
        Some(AnnIndex { map, indexed_count: store.entries.len() })
    }

    /// Approximate top-k ids, nearest first.
    pub fn search(idx: &AnnIndex, query: &[f32], top_k: usize) -> Vec<String> {
        let q = VecPoint(normalize(query));
        let mut search = Search::default();
        idx.map.search(&q, &mut search)
            .take(top_k)
            .map(|item| item.value.clone())
            .collect()
    }
}

/// Build the ANN index for fast search and store it on the handle.
/// Returns a JSON summary: how many entries were indexed and which backend is in use.
pub fn __varg_vector_build_index(store: &VectorStoreHandle) -> String {
    let mut s = store.lock().unwrap_or_else(|e| e.into_inner());
    let indexed = s.entries.len();
    #[cfg(feature = "ann")]
    {
        s.ann = ann_impl::build(&s);
        let built = s.ann.is_some();
        return serde_json::json!({
            "backend": "hnsw",
            "indexed": indexed,
            "built": built
        }).to_string();
    }
    #[cfg(not(feature = "ann"))]
    {
        let _ = &mut s;
        serde_json::json!({
            "backend": "exact",
            "indexed": indexed,
            "built": false
        }).to_string()
    }
}

/// Nearest-neighbour search. Uses the HNSW index when one has been built and is still current;
/// otherwise falls back to an exact scan (correct, linear) rather than silently returning
/// worse results from a stale index.
pub fn __varg_vector_search_fast<Q: ToF32Vec + ?Sized>(
    store: &VectorStoreHandle,
    query: &Q,
    top_k: i64,
) -> Vec<String> {
    let query = query.to_f32_vec();
    let query: &[f32] = &query;
    let s = store.lock().unwrap_or_else(|e| e.into_inner());
    let k = top_k.max(0) as usize;

    #[cfg(feature = "ann")]
    {
        if let Some(idx) = &s.ann {
            if idx.indexed_count == s.entries.len() {
                let ids = ann_impl::search(idx, query, k);
                // Report the true cosine score for each hit, ordered best-first.
                let mut scored: Vec<(String, f32)> = ids.into_iter()
                    .filter_map(|id| s.entries.iter().find(|e| e.id == id)
                        .map(|e| (id, cosine_similarity(query, &e.embedding))))
                    .collect();
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                return scored.into_iter()
                    .map(|(id, score)| serde_json::json!({"id": id, "score": score}).to_string())
                    .collect();
            }
        }
    }

    exact_top_k(&s, query, k)
        .into_iter()
        .map(|(id, score)| serde_json::json!({"id": id, "score": score}).to_string())
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

    /// The built index must be *kept* on the handle — the previous LSH implementation discarded it
    /// and rebuilt per query, which made "fast" search slower than a plain scan.
    #[cfg(feature = "ann")]
    #[test]
    fn test_ann_index_is_persisted_on_the_handle() {
        let store = __varg_vector_store_open(":memory:");
        for i in 0..20 {
            let v: Vec<f32> = (0..16).map(|j| ((i * 16 + j) as f32).sin()).collect();
            __varg_vector_store_upsert(&store, &format!("d{i}"), &v, &std::collections::HashMap::new());
        }
        assert!(store.lock().unwrap().ann.is_none(), "no index before build");
        let info = __varg_vector_build_index(&store);
        let v: serde_json::Value = serde_json::from_str(&info).unwrap();
        assert_eq!(v["backend"], "hnsw");
        assert_eq!(v["built"], true);
        let s = store.lock().unwrap();
        assert!(s.ann.is_some(), "index must be retained for reuse");
        assert_eq!(s.ann.as_ref().unwrap().indexed_count, 20);
    }

    /// Recall check: the HNSW hit must agree with the exact answer on a well-separated set.
    #[cfg(feature = "ann")]
    #[test]
    fn test_ann_search_matches_exact_top1() {
        let store = __varg_vector_store_open(":memory:");
        // 50 well-separated one-hot-ish vectors.
        for i in 0..50 {
            let mut v = vec![0.0f32; 50];
            v[i] = 1.0;
            __varg_vector_store_upsert(&store, &format!("d{i}"), &v, &std::collections::HashMap::new());
        }
        __varg_vector_build_index(&store);
        // Query close to d17.
        let mut q = vec![0.0f32; 50];
        q[17] = 0.9;
        q[3] = 0.1;
        let results = __varg_vector_search_fast(&store, &q, 1);
        let top: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(top["id"].as_str().unwrap(), "d17", "HNSW top-1 must match the exact nearest");
    }

    /// After mutating the store the index is stale; search must stay correct by falling back to an
    /// exact scan rather than answering from an out-of-date index.
    #[cfg(feature = "ann")]
    #[test]
    fn test_ann_stale_index_falls_back_to_exact() {
        let store = __varg_vector_store_open(":memory:");
        for i in 0..10 {
            let mut v = vec![0.0f32; 10];
            v[i] = 1.0;
            __varg_vector_store_upsert(&store, &format!("d{i}"), &v, &std::collections::HashMap::new());
        }
        __varg_vector_build_index(&store);
        // Add a new best match AFTER indexing — the index doesn't know about it.
        let mut newv = vec![0.0f32; 10];
        newv[0] = 1.0;
        newv[1] = 0.05;
        __varg_vector_store_upsert(&store, "newest", &newv, &std::collections::HashMap::new());

        let mut q = vec![0.0f32; 10];
        q[0] = 1.0;
        q[1] = 0.06;
        let results = __varg_vector_search_fast(&store, &q, 1);
        let top: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(top["id"].as_str().unwrap(), "newest",
            "stale index must not hide an entry added after the build");
    }

    #[test]
    fn test_ann_build_index_empty_store() {
        let store = __varg_vector_store_open(":memory:");
        let info = __varg_vector_build_index(&store);
        let v: serde_json::Value = serde_json::from_str(&info).unwrap();
        assert_eq!(v["indexed"], 0);
    }

    #[test]
    fn test_ann_build_index_with_entries() {
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
    fn test_ann_search_fast_returns_results() {
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
    fn test_ann_search_most_similar_first() {
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
