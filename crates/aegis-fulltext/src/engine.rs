//! The full-text engine: named indexes of `{id, text, metadata}` documents with
//! BM25-ranked search, exact-match metadata filtering, and snapshot persistence.

use crate::index::InvertedIndex;
use crate::tokenize::tokenize;
use crate::types::{FtsDocument, FtsError, SearchHit};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Summary stats for an index.
#[derive(Debug, Clone, Serialize)]
pub struct IndexStats {
    pub name: String,
    pub documents: usize,
}

struct StoredDoc {
    id: String,
    text: String,
    metadata: serde_json::Value,
    tokens: Vec<String>,
}

#[derive(Default)]
struct Index {
    inverted: InvertedIndex,
    id_to_doc: HashMap<String, u32>,
    docs: Vec<Option<StoredDoc>>,
}

impl Index {
    fn upsert(&mut self, id: String, text: String, metadata: serde_json::Value) {
        let tokens = tokenize(&text);
        if let Some(&old) = self.id_to_doc.get(&id) {
            if let Some(Some(prev)) = self.docs.get(old as usize) {
                let prev_tokens = prev.tokens.clone();
                self.inverted.remove(old, &prev_tokens);
            }
            self.docs[old as usize] = None;
        }
        let doc = self.docs.len() as u32;
        self.inverted.add(doc, &tokens);
        self.docs.push(Some(StoredDoc {
            id: id.clone(),
            text,
            metadata,
            tokens,
        }));
        self.id_to_doc.insert(id, doc);
    }

    fn get(&self, id: &str) -> Option<FtsDocument> {
        let &doc = self.id_to_doc.get(id)?;
        self.docs[doc as usize].as_ref().map(|d| FtsDocument {
            id: d.id.clone(),
            text: d.text.clone(),
            metadata: d.metadata.clone(),
        })
    }

    fn delete(&mut self, id: &str) -> bool {
        match self.id_to_doc.remove(id) {
            Some(doc) => {
                if let Some(Some(prev)) = self.docs.get(doc as usize) {
                    let tokens = prev.tokens.clone();
                    self.inverted.remove(doc, &tokens);
                }
                self.docs[doc as usize] = None;
                true
            }
            None => false,
        }
    }

    fn search(&self, query: &str, k: usize, filter: &serde_json::Value) -> Vec<SearchHit> {
        let tokens = tokenize(query);
        let scores = self.inverted.score(&tokens);
        let has_filter = filter.as_object().map(|m| !m.is_empty()).unwrap_or(false);

        let mut hits: Vec<SearchHit> = scores
            .into_iter()
            .filter_map(|(doc, score)| {
                let d = self.docs.get(doc as usize)?.as_ref()?;
                if has_filter && !matches_filter(&d.metadata, filter) {
                    return None;
                }
                Some(SearchHit {
                    id: d.id.clone(),
                    score,
                    metadata: d.metadata.clone(),
                })
            })
            .collect();
        hits.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
        hits.truncate(k);
        hits
    }

    fn documents(&self) -> Vec<FtsDocument> {
        self.docs
            .iter()
            .filter_map(|d| d.as_ref())
            .map(|d| FtsDocument {
                id: d.id.clone(),
                text: d.text.clone(),
                metadata: d.metadata.clone(),
            })
            .collect()
    }
}

/// Exact-match metadata filter (every key in `filter` must equal the document's).
fn matches_filter(meta: &serde_json::Value, filter: &serde_json::Value) -> bool {
    match (meta.as_object(), filter.as_object()) {
        (Some(m), Some(f)) => f.iter().all(|(k, v)| m.get(k) == Some(v)),
        (_, Some(f)) => f.is_empty(),
        _ => true,
    }
}

// ============================================================================
// Engine
// ============================================================================

/// Multi-index full-text engine.
#[derive(Default)]
pub struct FullTextEngine {
    indexes: RwLock<HashMap<String, Index>>,
}

impl FullTextEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_index(&self, name: impl Into<String>) -> Result<(), FtsError> {
        let name = name.into();
        let mut idx = self.indexes.write();
        if idx.contains_key(&name) {
            return Err(FtsError::IndexExists(name));
        }
        idx.insert(name, Index::default());
        Ok(())
    }

    pub fn drop_index(&self, name: &str) -> Result<(), FtsError> {
        self.indexes
            .write()
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| FtsError::IndexNotFound(name.to_string()))
    }

    pub fn list_indexes(&self) -> Vec<String> {
        let mut v: Vec<String> = self.indexes.read().keys().cloned().collect();
        v.sort();
        v
    }

    pub fn index_exists(&self, name: &str) -> bool {
        self.indexes.read().contains_key(name)
    }

    pub fn index_stats(&self, name: &str) -> Option<IndexStats> {
        let idx = self.indexes.read();
        let i = idx.get(name)?;
        Some(IndexStats {
            name: name.to_string(),
            documents: i.inverted.doc_count(),
        })
    }

    /// Index (insert or replace) a document.
    pub fn upsert(
        &self,
        index: &str,
        id: impl Into<String>,
        text: impl Into<String>,
        metadata: serde_json::Value,
    ) -> Result<(), FtsError> {
        let mut idx = self.indexes.write();
        let i = idx
            .get_mut(index)
            .ok_or_else(|| FtsError::IndexNotFound(index.to_string()))?;
        i.upsert(id.into(), text.into(), metadata);
        Ok(())
    }

    pub fn get(&self, index: &str, id: &str) -> Result<Option<FtsDocument>, FtsError> {
        let idx = self.indexes.read();
        let i = idx
            .get(index)
            .ok_or_else(|| FtsError::IndexNotFound(index.to_string()))?;
        Ok(i.get(id))
    }

    pub fn delete(&self, index: &str, id: &str) -> Result<bool, FtsError> {
        let mut idx = self.indexes.write();
        let i = idx
            .get_mut(index)
            .ok_or_else(|| FtsError::IndexNotFound(index.to_string()))?;
        Ok(i.delete(id))
    }

    pub fn search(
        &self,
        index: &str,
        query: &str,
        k: usize,
        filter: &serde_json::Value,
    ) -> Result<Vec<SearchHit>, FtsError> {
        let idx = self.indexes.read();
        let i = idx
            .get(index)
            .ok_or_else(|| FtsError::IndexNotFound(index.to_string()))?;
        Ok(i.search(query, k, filter))
    }

    // ---- Persistence ----------------------------------------------------

    pub fn snapshot(&self) -> EngineSnapshot {
        let idx = self.indexes.read();
        EngineSnapshot {
            indexes: idx
                .iter()
                .map(|(name, i)| IndexSnapshot {
                    name: name.clone(),
                    documents: i.documents(),
                })
                .collect(),
        }
    }

    pub fn load_snapshot(&self, snap: EngineSnapshot) {
        let mut idx = self.indexes.write();
        idx.clear();
        for is in snap.indexes {
            let mut index = Index::default();
            for d in is.documents {
                index.upsert(d.id, d.text, d.metadata);
            }
            idx.insert(is.name, index);
        }
    }
}

/// On-disk snapshot of the whole engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub indexes: Vec<IndexSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSnapshot {
    pub name: String,
    pub documents: Vec<FtsDocument>,
}
