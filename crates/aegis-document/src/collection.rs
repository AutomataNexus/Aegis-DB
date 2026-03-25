//! Aegis Document Collection
//!
//! Collection management for document storage.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::index::{DocumentIndex, IndexType};
use crate::query::{Filter, Query, QueryResult};
use crate::types::{Document, DocumentId};
use crate::validation::{Schema, ValidationResult};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

// =============================================================================
// Collection
// =============================================================================

/// A collection of documents.
pub struct Collection {
    name: String,
    documents: RwLock<HashMap<DocumentId, Document>>,
    indexes: RwLock<Vec<DocumentIndex>>,
    schema: Option<Schema>,
}

impl Collection {
    /// Create a new collection.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            documents: RwLock::new(HashMap::new()),
            indexes: RwLock::new(Vec::new()),
            schema: None,
        }
    }

    /// Create a collection with schema validation.
    pub fn with_schema(name: impl Into<String>, schema: Schema) -> Self {
        Self {
            name: name.into(),
            documents: RwLock::new(HashMap::new()),
            indexes: RwLock::new(Vec::new()),
            schema: Some(schema),
        }
    }

    /// Get the collection name.
    pub fn name(&self) -> &str {
        &self.name
    }

    // -------------------------------------------------------------------------
    // Document Operations
    // -------------------------------------------------------------------------

    /// Insert a document.
    pub fn insert(&self, doc: Document) -> Result<DocumentId, CollectionError> {
        if let Some(ref schema) = self.schema {
            let result = schema.validate(&doc);
            if !result.is_valid {
                return Err(CollectionError::ValidationFailed(result.errors));
            }
        }

        let id = doc.id.clone();

        {
            let mut docs = self.documents.write().expect("documents RwLock poisoned");
            if docs.contains_key(&id) {
                return Err(CollectionError::DuplicateId(id));
            }
            docs.insert(id.clone(), doc.clone());
        }

        self.index_document(&doc);

        Ok(id)
    }

    /// Insert multiple documents.
    pub fn insert_many(&self, docs: Vec<Document>) -> Result<Vec<DocumentId>, CollectionError> {
        let mut ids = Vec::with_capacity(docs.len());

        for doc in docs {
            let id = self.insert(doc)?;
            ids.push(id);
        }

        Ok(ids)
    }

    /// Get a document by ID.
    pub fn get(&self, id: &DocumentId) -> Option<Document> {
        let docs = self.documents.read().expect("documents RwLock poisoned");
        docs.get(id).cloned()
    }

    /// Update a document.
    pub fn update(&self, id: &DocumentId, doc: Document) -> Result<(), CollectionError> {
        if let Some(ref schema) = self.schema {
            let result = schema.validate(&doc);
            if !result.is_valid {
                return Err(CollectionError::ValidationFailed(result.errors));
            }
        }

        {
            let mut docs = self.documents.write().expect("documents RwLock poisoned");
            if !docs.contains_key(id) {
                return Err(CollectionError::NotFound(id.clone()));
            }

            if let Some(old_doc) = docs.get(id) {
                self.unindex_document(old_doc);
            }

            docs.insert(id.clone(), doc.clone());
        }

        self.index_document(&doc);

        Ok(())
    }

    /// Delete a document.
    pub fn delete(&self, id: &DocumentId) -> Result<Document, CollectionError> {
        let mut docs = self.documents.write().expect("documents RwLock poisoned");

        match docs.remove(id) {
            Some(doc) => {
                self.unindex_document(&doc);
                Ok(doc)
            }
            None => Err(CollectionError::NotFound(id.clone())),
        }
    }

    /// Check if a document exists.
    pub fn contains(&self, id: &DocumentId) -> bool {
        let docs = self.documents.read().expect("documents RwLock poisoned");
        docs.contains_key(id)
    }

    /// Get the number of documents.
    pub fn count(&self) -> usize {
        let docs = self.documents.read().expect("documents RwLock poisoned");
        docs.len()
    }

    /// Get all document IDs.
    pub fn ids(&self) -> Vec<DocumentId> {
        let docs = self.documents.read().expect("documents RwLock poisoned");
        docs.keys().cloned().collect()
    }

    /// Get all documents.
    pub fn all(&self) -> Vec<Document> {
        let docs = self.documents.read().expect("documents RwLock poisoned");
        docs.values().cloned().collect()
    }

    /// Clear all documents.
    pub fn clear(&self) {
        let mut docs = self.documents.write().expect("documents RwLock poisoned");
        docs.clear();

        let mut indexes = self.indexes.write().expect("indexes RwLock poisoned");
        for index in indexes.iter_mut() {
            index.clear();
        }
    }

    // -------------------------------------------------------------------------
    // Query Operations
    // -------------------------------------------------------------------------

    /// Find documents matching a query.
    ///
    /// When possible, uses indexes on fields with `Eq` filters to narrow
    /// the candidate set before applying the full filter chain, reducing
    /// the number of documents scanned.
    pub fn find(&self, query: &Query) -> QueryResult {
        let docs = self.documents.read().expect("documents RwLock poisoned");
        let indexes = self.indexes.read().expect("indexes RwLock poisoned");
        let start = std::time::Instant::now();

        // Try to find an Eq filter whose field has a hash/unique/btree index.
        // We pick the first matching index we find and use it to get candidate IDs.
        let candidate_ids = self.find_indexed_candidates(&query.filters, &indexes);

        let (mut matching, total_scanned) = if let Some(ids) = candidate_ids {
            // Index path: only scan the candidate documents
            let scanned = ids.len();
            let results: Vec<Document> = ids
                .iter()
                .filter_map(|id| docs.get(id))
                .filter(|doc| query.matches(doc))
                .cloned()
                .collect();
            (results, scanned)
        } else {
            // Full scan fallback
            let results: Vec<Document> = docs
                .values()
                .filter(|doc| query.matches(doc))
                .cloned()
                .collect();
            (results, docs.len())
        };

        // Apply sort
        if let Some(ref sort) = query.sort {
            matching.sort_by(|a, b| {
                let va = a.get(&sort.field);
                let vb = b.get(&sort.field);
                let ord = crate::types::compare_values(va, vb);
                if sort.ascending {
                    ord
                } else {
                    ord.reverse()
                }
            });
        }

        // Apply skip
        if let Some(skip) = query.skip {
            if skip < matching.len() {
                matching = matching.split_off(skip);
            } else {
                matching.clear();
            }
        }

        // Apply limit
        if let Some(limit) = query.limit {
            matching.truncate(limit);
        }

        // Apply projection
        if let Some(ref fields) = query.projection {
            for doc in &mut matching {
                doc.data.retain(|key, _| fields.contains(key));
            }
        }

        QueryResult {
            documents: matching,
            total_scanned,
            execution_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Find one document matching a query.
    pub fn find_one(&self, query: &Query) -> Option<Document> {
        let docs = self.documents.read().expect("documents RwLock poisoned");
        docs.values().find(|doc| query.matches(doc)).cloned()
    }

    /// Count documents matching a query.
    pub fn count_matching(&self, query: &Query) -> usize {
        let docs = self.documents.read().expect("documents RwLock poisoned");
        docs.values().filter(|doc| query.matches(doc)).count()
    }

    // -------------------------------------------------------------------------
    // Index-Accelerated Lookup (private)
    // -------------------------------------------------------------------------

    /// Scan filters for `Eq` conditions on indexed fields and return candidate
    /// document IDs from the first matching index. When multiple `Eq` filters
    /// have indexes, intersect their candidate sets for tighter filtering.
    /// Returns `None` if no applicable index is found (triggers full scan).
    fn find_indexed_candidates(
        &self,
        filters: &[Filter],
        indexes: &[DocumentIndex],
    ) -> Option<HashSet<DocumentId>> {
        let mut result: Option<HashSet<DocumentId>> = None;

        for filter in filters {
            if let Filter::Eq { field, value } = filter {
                // Find an index on this field (Hash, Unique, or BTree all use hash_index)
                if let Some(index) = indexes.iter().find(|idx| {
                    idx.field() == field.as_str()
                        && matches!(
                            idx.index_type(),
                            IndexType::Hash | IndexType::Unique | IndexType::BTree
                        )
                }) {
                    let ids: HashSet<DocumentId> = index.find_eq(value).into_iter().collect();
                    result = Some(match result {
                        Some(current) => current.intersection(&ids).cloned().collect(),
                        None => ids,
                    });
                }
            }
        }

        result
    }

    // -------------------------------------------------------------------------
    // Index Operations
    // -------------------------------------------------------------------------

    /// Create an index on a field.
    pub fn create_index(&self, field: impl Into<String>, index_type: IndexType) {
        let field = field.into();
        let mut index = DocumentIndex::new(field.clone(), index_type);

        let docs = self.documents.read().expect("documents RwLock poisoned");
        for doc in docs.values() {
            index.index_document(doc);
        }

        let mut indexes = self.indexes.write().expect("indexes RwLock poisoned");
        indexes.push(index);
    }

    /// Drop an index.
    pub fn drop_index(&self, field: &str) {
        let mut indexes = self.indexes.write().expect("indexes RwLock poisoned");
        indexes.retain(|idx| idx.field() != field);
    }

    /// Get all index names.
    pub fn index_names(&self) -> Vec<String> {
        let indexes = self.indexes.read().expect("indexes RwLock poisoned");
        indexes.iter().map(|idx| idx.field().to_string()).collect()
    }

    fn index_document(&self, doc: &Document) {
        let mut indexes = self.indexes.write().expect("indexes RwLock poisoned");
        for index in indexes.iter_mut() {
            index.index_document(doc);
        }
    }

    fn unindex_document(&self, doc: &Document) {
        let mut indexes = self.indexes.write().expect("indexes RwLock poisoned");
        for index in indexes.iter_mut() {
            index.unindex_document(doc);
        }
    }

    // -------------------------------------------------------------------------
    // Schema Operations
    // -------------------------------------------------------------------------

    /// Set the collection schema.
    pub fn set_schema(&mut self, schema: Schema) {
        self.schema = Some(schema);
    }

    /// Get the collection schema.
    pub fn schema(&self) -> Option<&Schema> {
        self.schema.as_ref()
    }

    /// Validate all documents against the schema.
    pub fn validate_all(&self) -> Vec<(DocumentId, ValidationResult)> {
        let Some(ref schema) = self.schema else {
            return Vec::new();
        };

        let docs = self.documents.read().expect("documents RwLock poisoned");
        docs.iter()
            .map(|(id, doc)| (id.clone(), schema.validate(doc)))
            .filter(|(_, result)| !result.is_valid)
            .collect()
    }
}

// =============================================================================
// Collection Error
// =============================================================================

/// Errors that can occur in collection operations.
#[derive(Debug, Clone)]
pub enum CollectionError {
    DuplicateId(DocumentId),
    NotFound(DocumentId),
    ValidationFailed(Vec<String>),
    IndexError(String),
}

impl std::fmt::Display for CollectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "Document with ID {} already exists", id),
            Self::NotFound(id) => write!(f, "Document with ID {} not found", id),
            Self::ValidationFailed(errors) => {
                write!(f, "Validation failed: {}", errors.join(", "))
            }
            Self::IndexError(msg) => write!(f, "Index error: {}", msg),
        }
    }
}

impl std::error::Error for CollectionError {}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::QueryBuilder;

    #[test]
    fn test_collection_creation() {
        let collection = Collection::new("users");
        assert_eq!(collection.name(), "users");
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn test_insert_and_get() {
        let collection = Collection::new("test");

        let mut doc = Document::with_id("doc1");
        doc.set("name", "Alice");

        let id = collection.insert(doc).unwrap();
        assert_eq!(id.as_str(), "doc1");

        let retrieved = collection.get(&id).unwrap();
        assert_eq!(
            retrieved.get("name").and_then(|v| v.as_str()),
            Some("Alice")
        );
    }

    #[test]
    fn test_duplicate_id() {
        let collection = Collection::new("test");

        let doc1 = Document::with_id("same-id");
        let doc2 = Document::with_id("same-id");

        collection.insert(doc1).unwrap();
        let result = collection.insert(doc2);

        assert!(matches!(result, Err(CollectionError::DuplicateId(_))));
    }

    #[test]
    fn test_update() {
        let collection = Collection::new("test");

        let mut doc = Document::with_id("doc1");
        doc.set("count", 1i64);
        collection.insert(doc).unwrap();

        let mut updated = Document::with_id("doc1");
        updated.set("count", 2i64);
        collection
            .update(&DocumentId::new("doc1"), updated)
            .unwrap();

        let retrieved = collection.get(&DocumentId::new("doc1")).unwrap();
        assert_eq!(retrieved.get("count").and_then(|v| v.as_i64()), Some(2));
    }

    #[test]
    fn test_delete() {
        let collection = Collection::new("test");

        let doc = Document::with_id("doc1");
        collection.insert(doc).unwrap();

        assert!(collection.contains(&DocumentId::new("doc1")));

        collection.delete(&DocumentId::new("doc1")).unwrap();
        assert!(!collection.contains(&DocumentId::new("doc1")));
    }

    #[test]
    fn test_find() {
        let collection = Collection::new("test");

        for i in 0..10 {
            let mut doc = Document::new();
            doc.set("value", i as i64);
            doc.set("even", i % 2 == 0);
            collection.insert(doc).unwrap();
        }

        let query = QueryBuilder::new().eq("even", true).build();
        let result = collection.find(&query);

        assert_eq!(result.documents.len(), 5);
    }

    #[test]
    fn test_find_uses_index() {
        let collection = Collection::new("test");

        // Insert 100 documents with a "status" field
        for i in 0..100 {
            let mut doc = Document::new();
            doc.set("status", if i % 10 == 0 { "active" } else { "inactive" });
            doc.set("value", i as i64);
            collection.insert(doc).unwrap();
        }

        // Query without index => full scan
        let query = QueryBuilder::new().eq("status", "active").build();
        let result_no_index = collection.find(&query);
        assert_eq!(result_no_index.documents.len(), 10);
        assert_eq!(result_no_index.total_scanned, 100); // full scan

        // Create index on "status"
        collection.create_index("status", IndexType::Hash);

        // Query with index => should scan far fewer documents
        let result_with_index = collection.find(&query);
        assert_eq!(result_with_index.documents.len(), 10);
        assert!(
            result_with_index.total_scanned < 100,
            "Expected index scan to examine fewer than 100 docs, got {}",
            result_with_index.total_scanned
        );
        // The index should return exactly the 10 matching documents
        assert_eq!(result_with_index.total_scanned, 10);
    }

    #[test]
    fn test_find_index_with_additional_filters() {
        let collection = Collection::new("test");

        // Insert docs: 50 active, 50 inactive; half of each have value > 25
        for i in 0..100 {
            let mut doc = Document::new();
            doc.set("status", if i < 50 { "active" } else { "inactive" });
            doc.set("value", i as i64);
            collection.insert(doc).unwrap();
        }

        collection.create_index("status", IndexType::Hash);

        // Eq filter on indexed field + Gt filter on non-indexed field
        let query = QueryBuilder::new()
            .eq("status", "active")
            .gt("value", 25i64)
            .build();

        let result = collection.find(&query);

        // active docs are i=0..50, value>25 means i=26..49 => 24 docs
        assert_eq!(result.documents.len(), 24);
        // Should only scan the 50 "active" candidates from the index
        assert_eq!(result.total_scanned, 50);
    }

    #[test]
    fn test_find_no_matching_index_falls_back_to_scan() {
        let collection = Collection::new("test");

        for i in 0..20 {
            let mut doc = Document::new();
            doc.set("x", i as i64);
            collection.insert(doc).unwrap();
        }

        // Index on a different field
        collection.create_index("y", IndexType::Hash);

        let query = QueryBuilder::new().eq("x", 5i64).build();
        let result = collection.find(&query);

        assert_eq!(result.documents.len(), 1);
        // No index on "x", so full scan
        assert_eq!(result.total_scanned, 20);
    }
}
