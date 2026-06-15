//! Aegis Full-Text — full-text search engine for the Aegis database.
//!
//! Named indexes of `{id, text, metadata}` documents with an inverted index and
//! Okapi **BM25** ranking, exact-match metadata filtering, exact deletes, and a
//! serializable snapshot for persistence.

pub mod engine;
pub mod index;
pub mod tokenize;
pub mod types;

pub use engine::{EngineSnapshot, FullTextEngine, IndexSnapshot, IndexStats};
pub use index::{Bm25Params, InvertedIndex};
pub use tokenize::tokenize;
pub use types::{FtsDocument, FtsError, SearchHit};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_auto_creates_index() {
        let e = FullTextEngine::new();
        // No create_index — the first upsert makes the index.
        e.upsert("auto", "d1", "hello world", serde_json::Value::Null)
            .unwrap();
        assert_eq!(e.list_indexes(), vec!["auto"]);
        let hits = e
            .search("auto", "hello", 10, &serde_json::Value::Null)
            .unwrap();
        assert_eq!(hits.len(), 1);
    }

    fn engine_with_docs() -> FullTextEngine {
        let e = FullTextEngine::new();
        e.create_index("docs").unwrap();
        e.upsert(
            "docs",
            "d1",
            "the quick brown fox jumps",
            serde_json::json!({"cat": "animal"}),
        )
        .unwrap();
        e.upsert(
            "docs",
            "d2",
            "aegis is a fast database written in rust",
            serde_json::json!({"cat": "tech"}),
        )
        .unwrap();
        e.upsert(
            "docs",
            "d3",
            "this database is a vector database for embeddings",
            serde_json::json!({"cat": "tech"}),
        )
        .unwrap();
        e
    }

    #[test]
    fn bm25_ranks_by_relevance() {
        let e = engine_with_docs();
        // "database" appears twice in d3, once in d2, not in d1.
        let hits = e
            .search("docs", "database", 10, &serde_json::Value::Null)
            .unwrap();
        let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["d3", "d2"],
            "BM25 should rank d3 above d2, exclude d1"
        );
        assert!(hits[0].score > hits[1].score);

        // multi-term query: d3 has both 'vector' and 'database'.
        let hits = e
            .search("docs", "vector database", 10, &serde_json::Value::Null)
            .unwrap();
        assert_eq!(hits[0].id, "d3");
    }

    #[test]
    fn stopwords_and_unknown_terms() {
        let e = engine_with_docs();
        // 'the' is a stopword, 'is' too -> a query of only stopwords matches nothing.
        let hits = e
            .search("docs", "the is a", 10, &serde_json::Value::Null)
            .unwrap();
        assert!(hits.is_empty());
        // unknown term -> no matches
        let hits = e
            .search("docs", "kubernetes", 10, &serde_json::Value::Null)
            .unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn metadata_filter() {
        let e = engine_with_docs();
        let hits = e
            .search("docs", "database", 10, &serde_json::json!({"cat": "tech"}))
            .unwrap();
        assert!(hits.iter().all(|h| h.metadata["cat"] == "tech"));
        assert!(!hits.is_empty());
    }

    #[test]
    fn upsert_replaces_and_delete_removes() {
        let e = engine_with_docs();
        assert_eq!(e.index_stats("docs").unwrap().documents, 3);

        // Re-upsert d2 with different text — it should no longer match 'database'.
        e.upsert(
            "docs",
            "d2",
            "aegis is a fast key value store",
            serde_json::Value::Null,
        )
        .unwrap();
        assert_eq!(e.index_stats("docs").unwrap().documents, 3);
        let ids: Vec<String> = e
            .search("docs", "database", 10, &serde_json::Value::Null)
            .unwrap()
            .into_iter()
            .map(|h| h.id)
            .collect();
        assert_eq!(ids, vec!["d3"]);

        // delete d3 -> 'database' now matches nothing.
        assert!(e.delete("docs", "d3").unwrap());
        assert_eq!(e.index_stats("docs").unwrap().documents, 2);
        assert!(e
            .search("docs", "database", 10, &serde_json::Value::Null)
            .unwrap()
            .is_empty());
        assert!(e.get("docs", "d3").unwrap().is_none());
    }

    #[test]
    fn missing_index_errors() {
        let e = FullTextEngine::new();
        assert!(matches!(
            e.search("nope", "x", 1, &serde_json::Value::Null),
            Err(FtsError::IndexNotFound(_))
        ));
    }

    #[test]
    fn snapshot_roundtrip() {
        let e = engine_with_docs();
        let bytes = serde_json::to_vec(&e.snapshot()).unwrap();

        let restored = FullTextEngine::new();
        restored.load_snapshot(serde_json::from_slice(&bytes).unwrap());
        assert_eq!(restored.index_stats("docs").unwrap().documents, 3);
        let hits = restored
            .search("docs", "database", 10, &serde_json::Value::Null)
            .unwrap();
        assert_eq!(hits[0].id, "d3");
    }
}
