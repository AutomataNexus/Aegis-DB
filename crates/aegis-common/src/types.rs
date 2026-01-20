//! Aegis Types - Core Data Types
//!
//! Fundamental data types used throughout the Aegis database platform.
//! Provides type-safe identifiers, value representations, and common
//! structures for storage and query operations.
//!
//! Key Features:
//! - Type-safe identifiers (BlockId, PageId, TransactionId, NodeId)
//! - Multi-paradigm value types (scalar, temporal, document, array)
//! - Compression and encryption type enumerations
//! - Serialization support via serde
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Identifier Types
// =============================================================================

/// Unique identifier for storage blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u64);

/// Unique identifier for buffer pool pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageId(pub u64);

/// Unique identifier for transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionId(pub u64);

/// Unique identifier for cluster nodes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// Unique identifier for shards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShardId(pub u32);

/// Log sequence number for WAL entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Lsn(pub u64);

// =============================================================================
// Block Types
// =============================================================================

/// Classification of storage block contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockType {
    TableData,
    TimeSeriesData,
    DocumentData,
    IndexData,
    LogEntry,
    Metadata,
}

/// Compression algorithm for stored data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CompressionType {
    #[default]
    None,
    Lz4,
    Zstd,
    Snappy,
}

/// Encryption algorithm for data at rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EncryptionType {
    #[default]
    None,
    Aes256Gcm,
}

// =============================================================================
// Value Types
// =============================================================================

/// Multi-paradigm value representation supporting all Aegis data models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    #[serde(with = "serde_bytes")]
    Bytes(Vec<u8>),
    Timestamp(DateTime<Utc>),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}

impl Value {
    /// Returns the type name of this value.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Boolean(_) => "boolean",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::Bytes(_) => "bytes",
            Value::Timestamp(_) => "timestamp",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    /// Returns true if this value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Attempts to extract a boolean value.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Attempts to extract an integer value.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Attempts to extract a float value.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Attempts to extract a string reference.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
}

// =============================================================================
// Data Type Definitions
// =============================================================================

/// SQL data type enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    Any,
    Boolean,
    TinyInt,
    SmallInt,
    Integer,
    BigInt,
    Float,
    Double,
    Decimal { precision: u8, scale: u8 },
    Char(u16),
    Varchar(u16),
    Text,
    Binary(u32),
    Varbinary(u32),
    Blob,
    Date,
    Time,
    Timestamp,
    TimestampTz,
    Interval,
    Json,
    Jsonb,
    Uuid,
    Array(Box<DataType>),
}

// =============================================================================
// Row and Column Types
// =============================================================================

/// A single row of data as a vector of values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub values: Vec<Value>,
}

impl Row {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Column metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default: Option<String>,
}

// =============================================================================
// Time Range
// =============================================================================

/// Time range for time series queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeRange {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    pub fn contains(&self, timestamp: &DateTime<Utc>) -> bool {
        timestamp >= &self.start && timestamp < &self.end
    }

    pub fn duration(&self) -> chrono::Duration {
        self.end - self.start
    }
}

// =============================================================================
// Key Range
// =============================================================================

/// Key range for shard boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRange {
    pub start: Option<Vec<u8>>,
    pub end: Option<Vec<u8>>,
}

impl KeyRange {
    pub fn new(start: Option<Vec<u8>>, end: Option<Vec<u8>>) -> Self {
        Self { start, end }
    }

    pub fn unbounded() -> Self {
        Self {
            start: None,
            end: None,
        }
    }

    pub fn contains(&self, key: &[u8]) -> bool {
        let after_start = self.start.as_ref().map_or(true, |s| key >= s.as_slice());
        let before_end = self.end.as_ref().map_or(true, |e| key < e.as_slice());
        after_start && before_end
    }
}
