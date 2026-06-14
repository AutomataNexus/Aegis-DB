//! Core types for the wide-column engine.

use serde::{Deserialize, Serialize};

/// A single cell: a value plus the timestamp of the write that set it.
/// Conflicting writes resolve last-write-wins by `timestamp` (ties keep the
/// existing value).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub value: serde_json::Value,
    pub timestamp: u64,
}

/// A row returned from a query: the row key plus its (projected) columns and
/// each column's write timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowResult {
    pub key: String,
    pub columns: serde_json::Map<String, serde_json::Value>,
    pub timestamps: serde_json::Map<String, serde_json::Value>,
}

/// Errors returned by the wide-column engine.
#[derive(Debug, thiserror::Error)]
pub enum WideColumnError {
    #[error("table '{0}' not found")]
    TableNotFound(String),
    #[error("table '{0}' already exists")]
    TableExists(String),
    #[error("a put must set at least one column")]
    EmptyWrite,
}
