// Wave 41: Full-text search builtins backed by tantivy (BM25)
//
// Simplified API: documents are (doc_id: string, body: string) pairs.
// fts_open(":memory:") creates an in-RAM index; any other path creates/opens on disk.
// fts_search returns doc_id strings ranked by BM25 score descending.

use tantivy::{
    doc,
    collector::TopDocs,
    query::QueryParser,
    schema::{Schema, STORED, STRING, TEXT, Field, Value},
    Index, IndexWriter, ReloadPolicy, TantivyDocument,
};
use std::sync::{Arc, Mutex};

pub struct FtsIndex {
    index:      Index,
    writer:     IndexWriter,
    id_field:   Field,
    body_field: Field,
}

pub type FtsHandle = Arc<Mutex<FtsIndex>>;

fn build_schema() -> (Schema, Field, Field) {
    let mut builder = Schema::builder();
    // id must be STRING (raw, untokenised) not TEXT, so `delete_term` matches the whole id
    // exactly. TEXT would tokenise e.g. "doc_to_delete" into separate terms and delete nothing.
    let id_field   = builder.add_text_field("id",   STRING | STORED);
    let body_field = builder.add_text_field("body", TEXT | STORED);
    (builder.build(), id_field, body_field)
}

pub fn __varg_fts_open(path: &str) -> Result<FtsHandle, String> {
    let (schema, id_field, body_field) = build_schema();
    let index = if path == ":memory:" {
        Index::create_in_ram(schema)
    } else {
        let p = std::path::Path::new(path);
        if p.exists() {
            Index::open_in_dir(p).map_err(|e| format!("fts_open: could not open existing index at path (the directory may be corrupted or locked by another process): {}", e))?
        } else {
            std::fs::create_dir_all(p).map_err(|e| format!("fts_open: could not create index directory (check that the path is valid and you have write permissions): {}", e))?;
            Index::create_in_dir(p, schema).map_err(|e| format!("fts_open: could not create a new index in the directory (check disk space and path permissions): {}", e))?
        }
    };
    let writer = index.writer(50_000_000).map_err(|e| format!("fts_open: could not allocate index writer (check disk space or path permissions): {}", e))?;
    Ok(Arc::new(Mutex::new(FtsIndex { index, writer, id_field, body_field })))
}

pub fn __varg_fts_add(handle: &FtsHandle, doc_id: &str, text: &str) -> Result<(), String> {
    let mut inner = handle.lock().unwrap_or_else(|e| e.into_inner());
    let id_field   = inner.id_field;
    let body_field = inner.body_field;
    inner.writer.add_document(doc!(
        id_field   => doc_id,
        body_field => text,
    )).map_err(|e| format!("fts_add: could not add document to index (the index writer may be in an invalid state): {}", e))?;
    Ok(())
}

pub fn __varg_fts_commit(handle: &FtsHandle) -> Result<(), String> {
    let mut inner = handle.lock().unwrap_or_else(|e| e.into_inner());
    inner.writer.commit().map_err(|e| format!("fts_commit: could not flush index changes to disk (check disk space and permissions): {}", e))?;
    Ok(())
}

pub fn __varg_fts_search(handle: &FtsHandle, query: &str, limit: i64) -> Result<Vec<String>, String> {
    let mut inner = handle.lock().unwrap_or_else(|e| e.into_inner());
    // Commit any pending changes first so they're visible
    inner.writer.commit().map_err(|e| format!("fts_search: could not commit pending writes before searching (check disk space): {}", e))?;
    let reader = inner.index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .map_err(|e| format!("fts_search: could not open index reader (the index may be corrupted): {}", e))?;
    let searcher = reader.searcher();
    let body_field = inner.body_field;
    let id_field   = inner.id_field;
    let query_parser = QueryParser::for_index(&inner.index, vec![body_field]);
    let parsed = query_parser.parse_query(query)
        // Cannot fail: `*` is a literal the parser always accepts. It is the fallback for
        // a user query that does not parse, which is deliberately lenient here.
        .unwrap_or_else(|_| query_parser.parse_query("*").unwrap());
    let top_docs = searcher.search(&parsed, &TopDocs::with_limit(limit.max(1) as usize))
        .map_err(|e| format!("fts_search: the search query could not be executed against the index (the index may be corrupted): {}", e))?;
    Ok(top_docs.into_iter().filter_map(|(_score, addr)| {
        // tantivy 0.22: Searcher::doc is generic over the document type — annotate it.
        let doc: TantivyDocument = searcher.doc(addr).ok()?;
        doc.get_first(id_field).and_then(|v| v.as_str().map(|s| s.to_string()))
    }).collect())
}

pub fn __varg_fts_delete(handle: &FtsHandle, doc_id: &str) -> Result<(), String> {
    let mut inner = handle.lock().unwrap_or_else(|e| e.into_inner());
    let id_field = inner.id_field;
    let term = tantivy::Term::from_field_text(id_field, doc_id);
    inner.writer.delete_term(term);
    inner.writer.commit().map_err(|e| format!("fts_delete: could not commit the deletion to disk (check disk space and permissions): {}", e))?;
    Ok(())
}

pub fn __varg_fts_close(handle: &FtsHandle) -> Result<(), String> {
    let mut inner = handle.lock().unwrap_or_else(|e| e.into_inner());
    inner.writer.commit().map_err(|e| format!("fts_close: could not flush final index changes before closing (check disk space and permissions): {}", e))?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fts_open_memory() {
        let h = __varg_fts_open(":memory:").unwrap();
        assert!(h.lock().is_ok());
    }

    #[test]
    fn test_fts_add_and_search() {
        let h = __varg_fts_open(":memory:").unwrap();
        __varg_fts_add(&h, "doc1", "the quick brown fox jumps over the lazy dog").unwrap();
        __varg_fts_add(&h, "doc2", "rust is a systems programming language").unwrap();
        __varg_fts_commit(&h).unwrap();
        let results = __varg_fts_search(&h, "fox", 10).unwrap();
        assert!(results.contains(&"doc1".to_string()), "fox query should return doc1");
    }

    #[test]
    fn test_fts_search_ranking() {
        let h = __varg_fts_open(":memory:").unwrap();
        __varg_fts_add(&h, "relevant", "quick brown fox quick quick fox").unwrap();
        __varg_fts_add(&h, "irrelevant", "the lazy dog").unwrap();
        __varg_fts_commit(&h).unwrap();
        let results = __varg_fts_search(&h, "fox", 10).unwrap();
        assert_eq!(results[0], "relevant", "most relevant doc should rank first");
    }

    #[test]
    fn test_fts_delete() {
        let h = __varg_fts_open(":memory:").unwrap();
        __varg_fts_add(&h, "doc_to_delete", "unique_term_xyz_abc").unwrap();
        __varg_fts_commit(&h).unwrap();
        // Verify it's there
        let before = __varg_fts_search(&h, "unique_term_xyz_abc", 10).unwrap();
        assert!(before.contains(&"doc_to_delete".to_string()));
        // Delete and verify gone
        __varg_fts_delete(&h, "doc_to_delete").unwrap();
        let after = __varg_fts_search(&h, "unique_term_xyz_abc", 10).unwrap();
        assert!(!after.contains(&"doc_to_delete".to_string()), "deleted doc should not appear");
    }

    #[test]
    fn test_fts_close_no_panic() {
        let h = __varg_fts_open(":memory:").unwrap();
        __varg_fts_add(&h, "d1", "some content").unwrap();
        __varg_fts_close(&h).unwrap(); // should not panic
    }

    #[test]
    fn test_fts_limit_respected() {
        let h = __varg_fts_open(":memory:").unwrap();
        for i in 0..10 {
            __varg_fts_add(&h, &format!("doc{}", i), "common search term").unwrap();
        }
        __varg_fts_commit(&h).unwrap();
        let results = __varg_fts_search(&h, "common", 3).unwrap();
        assert!(results.len() <= 3, "limit should be respected");
    }
}
