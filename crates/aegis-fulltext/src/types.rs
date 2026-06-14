//! Core types for the full-text engine.

use serde::{Deserialize, Serialize};

/// A document to be indexed: an opaque id, the text body, and JSON metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FtsDocument {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// A ranked search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: String,
    /// BM25 relevance score (higher = more relevant).
    pub score: f32,
    pub metadata: serde_json::Value,
}

/// Errors returned by the full-text engine.
#[derive(Debug, thiserror::Error)]
pub enum FtsError {
    #[error("index '{0}' not found")]
    IndexNotFound(String),
    #[error("index '{0}' already exists")]
    IndexExists(String),
    #[error("document '{0}' not found")]
    DocumentNotFound(String),
}
