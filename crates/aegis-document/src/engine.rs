//! Aegis Document Engine
//!
//! Core engine that coordinates all document store operations.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::collection::{Collection, CollectionError};
use crate::index::IndexType;
use crate::query::{Query, QueryResult};
use crate::types::{Document, DocumentId};
use crate::validation::Schema;
use std::collections::HashMap;
use std::sync::RwLock;

// =============================================================================
// Document Engine Configuration
// =============================================================================

/// Configuration for the document engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub max_document_size: usize,
    pub max_collections: usize,
    pub default_index_type: IndexType,
    pub validate_on_insert: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_document_size: 16 * 1024 * 1024, // 16MB
            max_collections: 1000,
            default_index_type: IndexType::Hash,
            validate_on_insert: true,
        }
    }
}

// =============================================================================
// Document Engine
// =============================================================================

/// The main document storage and query engine.
pub struct DocumentEngine {
    config: EngineConfig,
    collections: RwLock<HashMap<String, Collection>>,
    stats: RwLock<EngineStats>,
}

impl DocumentEngine {
    /// Create a new document engine with default configuration.
    pub fn new() -> Self {
        Self::with_config(EngineConfig::default())
    }

    /// Create a new document engine with custom configuration.
    pub fn with_config(config: EngineConfig) -> Self {
        Self {
            config,
            collections: RwLock::new(HashMap::new()),
            stats: RwLock::new(EngineStats::default()),
        }
    }

    // -------------------------------------------------------------------------
    // Collection Management
    // -------------------------------------------------------------------------

    /// Create a new collection.
    pub fn create_collection(&self, name: impl Into<String>) -> Result<(), EngineError> {
        let name = name.into();
        let mut collections = self.collections.write().unwrap();

        if collections.len() >= self.config.max_collections {
            return Err(EngineError::TooManyCollections);
        }

        if collections.contains_key(&name) {
            return Err(EngineError::CollectionExists(name));
        }

        collections.insert(name.clone(), Collection::new(name));
        Ok(())
    }

    /// Create a collection with a schema.
    pub fn create_collection_with_schema(
        &self,
        name: impl Into<String>,
        schema: Schema,
    ) -> Result<(), EngineError> {
        let name = name.into();
        let mut collections = self.collections.write().unwrap();

        if collections.len() >= self.config.max_collections {
            return Err(EngineError::TooManyCollections);
        }

        if collections.contains_key(&name) {
            return Err(EngineError::CollectionExists(name));
        }

        collections.insert(name.clone(), Collection::with_schema(name, schema));
        Ok(())
    }

    /// Drop a collection.
    pub fn drop_collection(&self, name: &str) -> Result<(), EngineError> {
        let mut collections = self.collections.write().unwrap();

        if collections.remove(name).is_none() {
            return Err(EngineError::CollectionNotFound(name.to_string()));
        }

        Ok(())
    }

    /// List all collection names.
    pub fn list_collections(&self) -> Vec<String> {
        let collections = self.collections.read().unwrap();
        collections.keys().cloned().collect()
    }

    /// Check if a collection exists.
    pub fn collection_exists(&self, name: &str) -> bool {
        let collections = self.collections.read().unwrap();
        collections.contains_key(name)
    }

    /// Get collection statistics.
    pub fn collection_stats(&self, name: &str) -> Option<CollectionStats> {
        let collections = self.collections.read().unwrap();
        collections.get(name).map(|c| CollectionStats {
            name: name.to_string(),
            document_count: c.count(),
            index_count: c.index_names().len(),
        })
    }

    // -------------------------------------------------------------------------
    // Document Operations
    // -------------------------------------------------------------------------

    /// Insert a document into a collection.
    pub fn insert(
        &self,
        collection: &str,
        doc: Document,
    ) -> Result<DocumentId, EngineError> {
        let collections = self.collections.read().unwrap();
        let coll = collections
            .get(collection)
            .ok_or_else(|| EngineError::CollectionNotFound(collection.to_string()))?;

        let id = coll.insert(doc).map_err(EngineError::Collection)?;

        drop(collections);

        {
            let mut stats = self.stats.write().unwrap();
            stats.documents_inserted += 1;
        }

        Ok(id)
    }

    /// Insert multiple documents.
    pub fn insert_many(
        &self,
        collection: &str,
        docs: Vec<Document>,
    ) -> Result<Vec<DocumentId>, EngineError> {
        let collections = self.collections.read().unwrap();
        let coll = collections
            .get(collection)
            .ok_or_else(|| EngineError::CollectionNotFound(collection.to_string()))?;

        let count = docs.len();
        let ids = coll.insert_many(docs).map_err(EngineError::Collection)?;

        drop(collections);

        {
            let mut stats = self.stats.write().unwrap();
            stats.documents_inserted += count as u64;
        }

        Ok(ids)
    }

    /// Get a document by ID.
    pub fn get(&self, collection: &str, id: &DocumentId) -> Result<Option<Document>, EngineError> {
        let collections = self.collections.read().unwrap();
        let coll = collections
            .get(collection)
            .ok_or_else(|| EngineError::CollectionNotFound(collection.to_string()))?;

        Ok(coll.get(id))
    }

    /// Update a document.
    pub fn update(
        &self,
        collection: &str,
        id: &DocumentId,
        doc: Document,
    ) -> Result<(), EngineError> {
        let collections = self.collections.read().unwrap();
        let coll = collections
            .get(collection)
            .ok_or_else(|| EngineError::CollectionNotFound(collection.to_string()))?;

        coll.update(id, doc).map_err(EngineError::Collection)?;

        drop(collections);

        {
            let mut stats = self.stats.write().unwrap();
            stats.documents_updated += 1;
        }

        Ok(())
    }

    /// Delete a document.
    pub fn delete(&self, collection: &str, id: &DocumentId) -> Result<Document, EngineError> {
        let collections = self.collections.read().unwrap();
        let coll = collections
            .get(collection)
            .ok_or_else(|| EngineError::CollectionNotFound(collection.to_string()))?;

        let doc = coll.delete(id).map_err(EngineError::Collection)?;

        drop(collections);

        {
            let mut stats = self.stats.write().unwrap();
            stats.documents_deleted += 1;
        }

        Ok(doc)
    }

    // -------------------------------------------------------------------------
    // Query Operations
    // -------------------------------------------------------------------------

    /// Find documents matching a query.
    pub fn find(&self, collection: &str, query: &Query) -> Result<QueryResult, EngineError> {
        let collections = self.collections.read().unwrap();
        let coll = collections
            .get(collection)
            .ok_or_else(|| EngineError::CollectionNotFound(collection.to_string()))?;

        let result = coll.find(query);

        drop(collections);

        {
            let mut stats = self.stats.write().unwrap();
            stats.queries_executed += 1;
        }

        Ok(result)
    }

    /// Find one document matching a query.
    pub fn find_one(&self, collection: &str, query: &Query) -> Result<Option<Document>, EngineError> {
        let collections = self.collections.read().unwrap();
        let coll = collections
            .get(collection)
            .ok_or_else(|| EngineError::CollectionNotFound(collection.to_string()))?;

        Ok(coll.find_one(query))
    }

    /// Count documents matching a query.
    pub fn count(&self, collection: &str, query: &Query) -> Result<usize, EngineError> {
        let collections = self.collections.read().unwrap();
        let coll = collections
            .get(collection)
            .ok_or_else(|| EngineError::CollectionNotFound(collection.to_string()))?;

        Ok(coll.count_matching(query))
    }

    // -------------------------------------------------------------------------
    // Index Operations
    // -------------------------------------------------------------------------

    /// Create an index on a collection field.
    pub fn create_index(
        &self,
        collection: &str,
        field: impl Into<String>,
        index_type: IndexType,
    ) -> Result<(), EngineError> {
        let collections = self.collections.read().unwrap();
        let coll = collections
            .get(collection)
            .ok_or_else(|| EngineError::CollectionNotFound(collection.to_string()))?;

        coll.create_index(field, index_type);
        Ok(())
    }

    /// Drop an index.
    pub fn drop_index(&self, collection: &str, field: &str) -> Result<(), EngineError> {
        let collections = self.collections.read().unwrap();
        let coll = collections
            .get(collection)
            .ok_or_else(|| EngineError::CollectionNotFound(collection.to_string()))?;

        coll.drop_index(field);
        Ok(())
    }

    /// List indexes on a collection.
    pub fn list_indexes(&self, collection: &str) -> Result<Vec<String>, EngineError> {
        let collections = self.collections.read().unwrap();
        let coll = collections
            .get(collection)
            .ok_or_else(|| EngineError::CollectionNotFound(collection.to_string()))?;

        Ok(coll.index_names())
    }

    // -------------------------------------------------------------------------
    // Statistics
    // -------------------------------------------------------------------------

    /// Get engine statistics.
    pub fn stats(&self) -> EngineStats {
        let stats = self.stats.read().unwrap();
        stats.clone()
    }

    /// Reset statistics.
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write().unwrap();
        *stats = EngineStats::default();
    }
}

impl Default for DocumentEngine {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Engine Statistics
// =============================================================================

/// Statistics for the document engine.
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    pub documents_inserted: u64,
    pub documents_updated: u64,
    pub documents_deleted: u64,
    pub queries_executed: u64,
}

/// Statistics for a collection.
#[derive(Debug, Clone)]
pub struct CollectionStats {
    pub name: String,
    pub document_count: usize,
    pub index_count: usize,
}

// =============================================================================
// Engine Error
// =============================================================================

/// Errors that can occur in the document engine.
#[derive(Debug, Clone)]
pub enum EngineError {
    CollectionExists(String),
    CollectionNotFound(String),
    TooManyCollections,
    Collection(CollectionError),
    DocumentTooLarge,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CollectionExists(name) => write!(f, "Collection already exists: {}", name),
            Self::CollectionNotFound(name) => write!(f, "Collection not found: {}", name),
            Self::TooManyCollections => write!(f, "Maximum number of collections reached"),
            Self::Collection(err) => write!(f, "Collection error: {}", err),
            Self::DocumentTooLarge => write!(f, "Document exceeds maximum size"),
        }
    }
}

impl std::error::Error for EngineError {}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::QueryBuilder;

    #[test]
    fn test_engine_creation() {
        let engine = DocumentEngine::new();
        assert!(engine.list_collections().is_empty());
    }

    #[test]
    fn test_collection_management() {
        let engine = DocumentEngine::new();

        engine.create_collection("users").unwrap();
        assert!(engine.collection_exists("users"));

        let collections = engine.list_collections();
        assert_eq!(collections.len(), 1);
        assert!(collections.contains(&"users".to_string()));

        engine.drop_collection("users").unwrap();
        assert!(!engine.collection_exists("users"));
    }

    #[test]
    fn test_document_crud() {
        let engine = DocumentEngine::new();
        engine.create_collection("test").unwrap();

        let mut doc = Document::with_id("doc1");
        doc.set("name", "Alice");
        doc.set("age", 30i64);

        let id = engine.insert("test", doc).unwrap();
        assert_eq!(id.as_str(), "doc1");

        let retrieved = engine.get("test", &id).unwrap().unwrap();
        assert_eq!(retrieved.get("name").and_then(|v| v.as_str()), Some("Alice"));

        let mut updated = Document::with_id("doc1");
        updated.set("name", "Alice Smith");
        updated.set("age", 31i64);
        engine.update("test", &id, updated).unwrap();

        let retrieved = engine.get("test", &id).unwrap().unwrap();
        assert_eq!(
            retrieved.get("name").and_then(|v| v.as_str()),
            Some("Alice Smith")
        );

        engine.delete("test", &id).unwrap();
        assert!(engine.get("test", &id).unwrap().is_none());
    }

    #[test]
    fn test_query() {
        let engine = DocumentEngine::new();
        engine.create_collection("products").unwrap();

        for i in 0..10 {
            let mut doc = Document::new();
            doc.set("name", format!("Product {}", i));
            doc.set("price", (i * 10) as i64);
            doc.set("in_stock", i % 2 == 0);
            engine.insert("products", doc).unwrap();
        }

        let query = QueryBuilder::new().eq("in_stock", true).build();
        let result = engine.find("products", &query).unwrap();
        assert_eq!(result.count(), 5);

        let query = QueryBuilder::new().gt("price", 50i64).build();
        let result = engine.find("products", &query).unwrap();
        assert_eq!(result.count(), 4);
    }

    #[test]
    fn test_index() {
        let engine = DocumentEngine::new();
        engine.create_collection("items").unwrap();

        engine
            .create_index("items", "category", IndexType::Hash)
            .unwrap();

        let indexes = engine.list_indexes("items").unwrap();
        assert!(indexes.contains(&"category".to_string()));

        engine.drop_index("items", "category").unwrap();
        let indexes = engine.list_indexes("items").unwrap();
        assert!(!indexes.contains(&"category".to_string()));
    }

    #[test]
    fn test_stats() {
        let engine = DocumentEngine::new();
        engine.create_collection("test").unwrap();

        for _ in 0..5 {
            let doc = Document::new();
            engine.insert("test", doc).unwrap();
        }

        let stats = engine.stats();
        assert_eq!(stats.documents_inserted, 5);
    }
}
