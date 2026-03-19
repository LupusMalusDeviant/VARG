// Wave 21: Agent Memory Runtime
//
// 3-layer memory architecture:
// 1. Working Memory — HashMap for current session context (fast, ephemeral)
// 2. Episodic Memory — Vector store for past interactions (similarity search)
// 3. Semantic Memory — Knowledge graph for facts and relations (structured)
//
// No external dependencies — builds on Wave 20 (graph) and 20b (vector).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::graph::{GraphHandle, __varg_graph_open, __varg_graph_add_node, __varg_graph_query};
use crate::vector::{VectorStoreHandle, __varg_vector_store_open, __varg_vector_store_upsert,
                    __varg_vector_store_search, __varg_embed, __varg_vector_store_count};

static MEMORY_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct AgentMemory {
    pub name: String,
    /// Layer 1: Working memory — key-value pairs for current session
    pub working: HashMap<String, String>,
    /// Layer 2: Episodic memory — vector store for past interactions
    pub episodic: VectorStoreHandle,
    /// Layer 3: Semantic memory — knowledge graph for facts
    pub semantic: GraphHandle,
}

/// Shared, thread-safe memory handle
pub type MemoryHandle = Arc<Mutex<AgentMemory>>;

/// Open or create a named agent memory (all 3 layers)
pub fn __varg_memory_open(name: &str) -> MemoryHandle {
    Arc::new(Mutex::new(AgentMemory {
        name: name.to_string(),
        working: HashMap::new(),
        episodic: __varg_vector_store_open(&format!("{}_episodic", name)),
        semantic: __varg_graph_open(&format!("{}_semantic", name)),
    }))
}

/// Store a key-value pair in working memory
pub fn __varg_memory_set(mem: &MemoryHandle, key: &str, value: &str) {
    mem.lock().unwrap().working.insert(key.to_string(), value.to_string());
}

/// Get a value from working memory
pub fn __varg_memory_get(mem: &MemoryHandle, key: &str, default: &str) -> String {
    mem.lock().unwrap().working.get(key).cloned().unwrap_or_else(|| default.to_string())
}

/// Store an interaction in episodic memory (auto-embedded)
pub fn __varg_memory_store(mem: &MemoryHandle, content: &str, metadata: &HashMap<String, String>) {
    let id = format!("ep_{}", MEMORY_COUNTER.fetch_add(1, Ordering::SeqCst));
    let embedding = __varg_embed(content);
    let mut meta = metadata.clone();
    meta.insert("_content".to_string(), content.to_string());
    meta.insert("_timestamp".to_string(), format!("{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()));
    let m = mem.lock().unwrap();
    __varg_vector_store_upsert(&m.episodic, &id, &embedding, &meta);
}

/// Recall relevant memories by similarity search across episodic memory
pub fn __varg_memory_recall(
    mem: &MemoryHandle,
    query_text: &str,
    top_k: i64,
) -> Vec<HashMap<String, String>> {
    let query_embedding = __varg_embed(query_text);
    let m = mem.lock().unwrap();
    __varg_vector_store_search(&m.episodic, &query_embedding, top_k)
}

/// Store a fact in semantic memory (knowledge graph)
pub fn __varg_memory_add_fact(
    mem: &MemoryHandle,
    label: &str,
    props: &HashMap<String, String>,
) -> i64 {
    let m = mem.lock().unwrap();
    __varg_graph_add_node(&m.semantic, label, props)
}

/// Query facts from semantic memory by label
pub fn __varg_memory_query_facts(
    mem: &MemoryHandle,
    label: &str,
) -> Vec<HashMap<String, String>> {
    let m = mem.lock().unwrap();
    __varg_graph_query(&m.semantic, label)
}

/// Get episodic memory count
pub fn __varg_memory_episode_count(mem: &MemoryHandle) -> i64 {
    let m = mem.lock().unwrap();
    __varg_vector_store_count(&m.episodic)
}

/// Clear working memory
pub fn __varg_memory_clear_working(mem: &MemoryHandle) {
    mem.lock().unwrap().working.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_open() {
        let mem = __varg_memory_open("test_agent");
        let m = mem.lock().unwrap();
        assert_eq!(m.name, "test_agent");
        assert!(m.working.is_empty());
    }

    #[test]
    fn test_memory_working_set_get() {
        let mem = __varg_memory_open("test");
        __varg_memory_set(&mem, "task", "implement wave 21");
        let val = __varg_memory_get(&mem, "task", "none");
        assert_eq!(val, "implement wave 21");
        let def = __varg_memory_get(&mem, "missing", "default");
        assert_eq!(def, "default");
    }

    #[test]
    fn test_memory_store_and_recall() {
        let mem = __varg_memory_open("test");
        let meta = HashMap::new();
        __varg_memory_store(&mem, "The user asked about Rust performance", &meta);
        __varg_memory_store(&mem, "We discussed compiler optimization techniques", &meta);
        __varg_memory_store(&mem, "The weather is sunny today", &meta);

        assert_eq!(__varg_memory_episode_count(&mem), 3);

        let results = __varg_memory_recall(&mem, "Rust compiler", 2);
        assert_eq!(results.len(), 2);
        // Results should have _content field
        assert!(results[0].contains_key("_content"));
    }

    #[test]
    fn test_memory_semantic_facts() {
        let mem = __varg_memory_open("test");
        let props = HashMap::from([("name".to_string(), "Varg".to_string())]);
        let id = __varg_memory_add_fact(&mem, "Project", &props);
        assert!(id > 0);

        let facts = __varg_memory_query_facts(&mem, "Project");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].get("name").unwrap(), "Varg");
    }

    #[test]
    fn test_memory_clear_working() {
        let mem = __varg_memory_open("test");
        __varg_memory_set(&mem, "a", "1");
        __varg_memory_set(&mem, "b", "2");
        __varg_memory_clear_working(&mem);
        let val = __varg_memory_get(&mem, "a", "gone");
        assert_eq!(val, "gone");
    }
}
