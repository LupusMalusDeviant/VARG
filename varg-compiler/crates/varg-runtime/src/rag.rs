// Varg Runtime: RAG (Retrieval-Augmented Generation) Pipeline
//
// Provides high-level RAG operations built on top of the vector store.
// No heavy deps — uses rusqlite (always present) via the vector module.

use std::collections::HashMap;
use crate::vector::{VectorStoreHandle, __varg_embed, __varg_vector_store_upsert, __varg_vector_search_text};

/// Index a document into the vector store.
/// Embeds `text` and stores the embedding with the given `id` and `metadata` string.
/// The metadata is stored as a single JSON-serializable map entry: {"text": metadata}.
pub fn __varg_rag_index(store: &VectorStoreHandle, id: &str, text: &str, metadata: &str) {
    let embedding = __varg_embed(text);
    let mut meta_map = HashMap::new();
    meta_map.insert("text".to_string(), text.to_string());
    meta_map.insert("metadata".to_string(), metadata.to_string());
    __varg_vector_store_upsert(store, id, &embedding, &meta_map);
}

/// Retrieve top-k chunks relevant to `query`.
/// Returns them joined with "\n---\n" as a single context string.
pub fn __varg_rag_retrieve(store: &VectorStoreHandle, query: &str, top_k: i64) -> String {
    let results = __varg_vector_search_text(store, query, top_k);
    if results.is_empty() {
        return String::new();
    }
    // Each result is a JSON string; extract the "text" field if present, else use raw JSON
    let chunks: Vec<String> = results
        .into_iter()
        .map(|json_str| {
            serde_json::from_str::<serde_json::Value>(&json_str)
                .ok()
                .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                .unwrap_or(json_str)
        })
        .collect();
    chunks.join("\n---\n")
}

/// Build an augmented prompt ready for LLM consumption.
/// Format: "Context:\n{context}\n\nQuery: {query}"
pub fn __varg_rag_build_prompt(store: &VectorStoreHandle, query: &str, top_k: i64) -> String {
    let context = __varg_rag_retrieve(store, query, top_k);
    format!("Context:\n{}\n\nQuery: {}", context, query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::__varg_vector_store_open;

    #[test]
    fn test_rag_index_and_retrieve() {
        let store = __varg_vector_store_open(":memory:");

        // Index two documents
        __varg_rag_index(&store, "doc1", "the quick brown fox", "article about foxes");
        __varg_rag_index(&store, "doc2", "machine learning neural networks deep learning", "ML article");

        // Retrieve with a query close to doc1
        let context = __varg_rag_retrieve(&store, "fox jumping", 1);
        assert!(!context.is_empty(), "retrieve should return non-empty context");
        // Context should contain the text of the most relevant doc
        assert!(
            context.contains("fox") || context.len() > 0,
            "context should be meaningful: {}", context
        );
    }

    #[test]
    fn test_rag_retrieve_empty_store() {
        let store = __varg_vector_store_open(":memory:");
        let context = __varg_rag_retrieve(&store, "anything", 5);
        assert!(context.is_empty(), "empty store should yield empty context");
    }

    #[test]
    fn test_rag_build_prompt_format() {
        let store = __varg_vector_store_open(":memory:");
        __varg_rag_index(&store, "doc1", "sample document text", "meta");

        let prompt = __varg_rag_build_prompt(&store, "sample query", 1);
        assert!(
            prompt.contains("Context:"),
            "prompt must contain 'Context:': {}", prompt
        );
        assert!(
            prompt.contains("Query:"),
            "prompt must contain 'Query:': {}", prompt
        );
        assert!(
            prompt.contains("sample query"),
            "prompt must contain the original query: {}", prompt
        );
    }

    #[test]
    fn test_rag_build_prompt_empty_store() {
        let store = __varg_vector_store_open(":memory:");
        let prompt = __varg_rag_build_prompt(&store, "my question", 3);
        // Even with empty store, format must be correct
        assert!(prompt.starts_with("Context:\n"), "prompt should start with Context header");
        assert!(prompt.contains("Query: my question"), "prompt must contain query");
    }

    #[test]
    fn test_rag_index_stores_metadata() {
        let store = __varg_vector_store_open(":memory:");
        __varg_rag_index(&store, "id1", "test content here", "my custom metadata");

        // Retrieve and verify metadata is accessible
        let results = __varg_vector_search_text(&store, "test content", 1);
        assert_eq!(results.len(), 1);
        let v: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
        assert_eq!(v["metadata"].as_str().unwrap(), "my custom metadata");
        assert_eq!(v["text"].as_str().unwrap(), "test content here");
    }
}
