//! Aegis Document Index
//!
//! Indexing structures for efficient document queries.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::types::{Document, DocumentId, Value};
use std::collections::{HashMap, HashSet};

// =============================================================================
// Index Type
// =============================================================================

/// Type of document index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    /// Hash index for equality lookups.
    Hash,
    /// B-tree index for range queries.
    BTree,
    /// Full-text search index.
    FullText,
    /// Unique index (no duplicates allowed).
    Unique,
}

// =============================================================================
// Document Index
// =============================================================================

/// Index for efficient document queries.
pub struct DocumentIndex {
    field: String,
    index_type: IndexType,
    hash_index: HashMap<IndexKey, HashSet<DocumentId>>,
    text_index: Option<InvertedIndex>,
}

impl DocumentIndex {
    /// Create a new index.
    pub fn new(field: impl Into<String>, index_type: IndexType) -> Self {
        let text_index = if index_type == IndexType::FullText {
            Some(InvertedIndex::new())
        } else {
            None
        };

        Self {
            field: field.into(),
            index_type,
            hash_index: HashMap::new(),
            text_index,
        }
    }

    /// Get the indexed field name.
    pub fn field(&self) -> &str {
        &self.field
    }

    /// Get the index type.
    pub fn index_type(&self) -> IndexType {
        self.index_type
    }

    /// Index a document.
    pub fn index_document(&mut self, doc: &Document) {
        let value = doc.get(&self.field);

        if let Some(value) = value {
            match self.index_type {
                IndexType::FullText => {
                    if let Some(text) = value.as_str() {
                        if let Some(ref mut idx) = self.text_index {
                            idx.index_document(&doc.id, text);
                        }
                    }
                }
                _ => {
                    let key = IndexKey::from_value(value);
                    self.hash_index
                        .entry(key)
                        .or_default()
                        .insert(doc.id.clone());
                }
            }
        }
    }

    /// Remove a document from the index.
    pub fn unindex_document(&mut self, doc: &Document) {
        let value = doc.get(&self.field);

        if let Some(value) = value {
            match self.index_type {
                IndexType::FullText => {
                    if let Some(text) = value.as_str() {
                        if let Some(ref mut idx) = self.text_index {
                            idx.unindex_document(&doc.id, text);
                        }
                    }
                }
                _ => {
                    let key = IndexKey::from_value(value);
                    if let Some(ids) = self.hash_index.get_mut(&key) {
                        ids.remove(&doc.id);
                        if ids.is_empty() {
                            self.hash_index.remove(&key);
                        }
                    }
                }
            }
        }
    }

    /// Find documents by exact value.
    pub fn find_eq(&self, value: &Value) -> Vec<DocumentId> {
        let key = IndexKey::from_value(value);
        self.hash_index
            .get(&key)
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Search full-text index.
    pub fn search(&self, query: &str) -> Vec<DocumentId> {
        self.text_index
            .as_ref()
            .map(|idx| idx.search(query))
            .unwrap_or_default()
    }

    /// Clear the index.
    pub fn clear(&mut self) {
        self.hash_index.clear();
        if let Some(ref mut idx) = self.text_index {
            idx.clear();
        }
    }

    /// Get the number of unique keys.
    pub fn key_count(&self) -> usize {
        self.hash_index.len()
    }
}

// =============================================================================
// Index Key
// =============================================================================

/// Key for hash-based indexing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum IndexKey {
    Null,
    Bool(bool),
    Int(i64),
    String(String),
    Float(OrderedFloat),
}

impl IndexKey {
    fn from_value(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(b) => Self::Bool(*b),
            Value::Int(n) => Self::Int(*n),
            Value::Float(f) => Self::Float(OrderedFloat(*f)),
            Value::String(s) => Self::String(s.clone()),
            Value::Array(_) | Value::Object(_) => Self::Null,
        }
    }
}

/// Wrapper for f64 that implements Eq and Hash.
#[derive(Debug, Clone, Copy)]
struct OrderedFloat(f64);

impl PartialEq for OrderedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for OrderedFloat {}

impl std::hash::Hash for OrderedFloat {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

// =============================================================================
// Inverted Index
// =============================================================================

/// Inverted index for full-text search.
pub struct InvertedIndex {
    terms: HashMap<String, HashSet<DocumentId>>,
    doc_terms: HashMap<DocumentId, HashSet<String>>,
}

impl InvertedIndex {
    pub fn new() -> Self {
        Self {
            terms: HashMap::new(),
            doc_terms: HashMap::new(),
        }
    }

    /// Index a document's text.
    pub fn index_document(&mut self, doc_id: &DocumentId, text: &str) {
        let tokens = self.tokenize(text);
        let mut doc_term_set = HashSet::new();

        for token in tokens {
            self.terms
                .entry(token.clone())
                .or_default()
                .insert(doc_id.clone());
            doc_term_set.insert(token);
        }

        self.doc_terms.insert(doc_id.clone(), doc_term_set);
    }

    /// Remove a document from the index.
    pub fn unindex_document(&mut self, doc_id: &DocumentId, text: &str) {
        let tokens = self.tokenize(text);

        for token in &tokens {
            if let Some(docs) = self.terms.get_mut(token) {
                docs.remove(doc_id);
                if docs.is_empty() {
                    self.terms.remove(token);
                }
            }
        }

        self.doc_terms.remove(doc_id);
    }

    /// Search for documents containing the query terms.
    pub fn search(&self, query: &str) -> Vec<DocumentId> {
        let tokens = self.tokenize(query);

        if tokens.is_empty() {
            return Vec::new();
        }

        let mut result: Option<HashSet<DocumentId>> = None;

        for token in tokens {
            let matching = self.terms.get(&token).cloned().unwrap_or_default();

            result = Some(match result {
                Some(current) => current.intersection(&matching).cloned().collect(),
                None => matching,
            });
        }

        result
            .map(|set| set.into_iter().collect())
            .unwrap_or_default()
    }

    /// Search with OR semantics.
    pub fn search_any(&self, query: &str) -> Vec<DocumentId> {
        let tokens = self.tokenize(query);
        let mut result = HashSet::new();

        for token in tokens {
            if let Some(docs) = self.terms.get(&token) {
                result.extend(docs.iter().cloned());
            }
        }

        result.into_iter().collect()
    }

    /// Clear the index.
    pub fn clear(&mut self) {
        self.terms.clear();
        self.doc_terms.clear();
    }

    /// Get the number of unique terms.
    pub fn term_count(&self) -> usize {
        self.terms.len()
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() >= 2)
            .map(String::from)
            .collect()
    }
}

impl Default for InvertedIndex {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_index() {
        let mut index = DocumentIndex::new("status", IndexType::Hash);

        let mut doc1 = Document::with_id("doc1");
        doc1.set("status", "active");

        let mut doc2 = Document::with_id("doc2");
        doc2.set("status", "active");

        let mut doc3 = Document::with_id("doc3");
        doc3.set("status", "inactive");

        index.index_document(&doc1);
        index.index_document(&doc2);
        index.index_document(&doc3);

        let result = index.find_eq(&Value::String("active".to_string()));
        assert_eq!(result.len(), 2);

        let result = index.find_eq(&Value::String("inactive".to_string()));
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_inverted_index() {
        let mut index = InvertedIndex::new();

        let doc1 = DocumentId::new("doc1");
        let doc2 = DocumentId::new("doc2");

        index.index_document(&doc1, "The quick brown fox");
        index.index_document(&doc2, "The lazy brown dog");

        let result = index.search("brown");
        assert_eq!(result.len(), 2);

        let result = index.search("quick");
        assert_eq!(result.len(), 1);

        let result = index.search("quick brown");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_full_text_index() {
        let mut index = DocumentIndex::new("content", IndexType::FullText);

        let mut doc1 = Document::with_id("doc1");
        doc1.set("content", "Hello world");

        let mut doc2 = Document::with_id("doc2");
        doc2.set("content", "Goodbye world");

        index.index_document(&doc1);
        index.index_document(&doc2);

        let result = index.search("world");
        assert_eq!(result.len(), 2);

        let result = index.search("hello");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_unindex() {
        let mut index = DocumentIndex::new("name", IndexType::Hash);

        let mut doc = Document::with_id("doc1");
        doc.set("name", "Test");

        index.index_document(&doc);
        assert_eq!(index.find_eq(&Value::String("Test".to_string())).len(), 1);

        index.unindex_document(&doc);
        assert_eq!(index.find_eq(&Value::String("Test".to_string())).len(), 0);
    }
}
