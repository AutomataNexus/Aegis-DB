//! Aegis Client Query Results
//!
//! Types for query result handling.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Value
// =============================================================================

/// A database value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
    Timestamp(i64),
}

impl Value {
    /// Check if the value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Try to get as bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as i64.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    /// Try to get as f64.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Try to get as string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as bytes.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Try to get as array.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Try to get as object.
    pub fn as_object(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Int(v as i64)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::String(v.to_string())
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(v)
    }
}

// =============================================================================
// Column
// =============================================================================

/// Metadata about a result column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

impl Column {
    pub fn new(name: impl Into<String>, data_type: DataType) -> Self {
        Self {
            name: name.into(),
            data_type,
            nullable: true,
        }
    }
}

// =============================================================================
// Data Type
// =============================================================================

/// SQL data types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    Boolean,
    Integer,
    BigInt,
    Float,
    Double,
    Text,
    Varchar,
    Blob,
    Timestamp,
    Date,
    Time,
    Json,
    Array,
    Unknown,
}

// =============================================================================
// Row
// =============================================================================

/// A row in a query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    columns: Vec<String>,
    values: Vec<Value>,
}

impl Row {
    /// Create a new row.
    pub fn new(columns: Vec<String>, values: Vec<Value>) -> Self {
        Self { columns, values }
    }

    /// Get a value by column index.
    pub fn get(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    /// Get a value by column name.
    pub fn get_by_name(&self, name: &str) -> Option<&Value> {
        let index = self.columns.iter().position(|c| c == name)?;
        self.values.get(index)
    }

    /// Get the number of columns.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if the row is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Get column names.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// Get all values.
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// Try to get a bool value.
    pub fn get_bool(&self, index: usize) -> Option<bool> {
        self.get(index).and_then(|v| v.as_bool())
    }

    /// Try to get an int value.
    pub fn get_int(&self, index: usize) -> Option<i64> {
        self.get(index).and_then(|v| v.as_int())
    }

    /// Try to get a float value.
    pub fn get_float(&self, index: usize) -> Option<f64> {
        self.get(index).and_then(|v| v.as_float())
    }

    /// Try to get a string value.
    pub fn get_str(&self, index: usize) -> Option<&str> {
        self.get(index).and_then(|v| v.as_str())
    }

    /// Convert to a HashMap.
    pub fn to_map(&self) -> HashMap<String, Value> {
        self.columns
            .iter()
            .zip(self.values.iter())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

impl IntoIterator for Row {
    type Item = Value;
    type IntoIter = std::vec::IntoIter<Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}

// =============================================================================
// Query Result
// =============================================================================

/// Result of a query execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    columns: Vec<Column>,
    rows: Vec<Row>,
    rows_affected: u64,
    execution_time_ms: u64,
}

impl QueryResult {
    /// Create a new query result.
    pub fn new(columns: Vec<Column>, rows: Vec<Row>) -> Self {
        Self {
            columns,
            rows,
            rows_affected: 0,
            execution_time_ms: 0,
        }
    }

    /// Create an empty result.
    pub fn empty() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: 0,
            execution_time_ms: 0,
        }
    }

    /// Create a result for a write operation.
    pub fn affected(rows_affected: u64) -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected,
            execution_time_ms: 0,
        }
    }

    /// Set execution time.
    pub fn with_execution_time(mut self, ms: u64) -> Self {
        self.execution_time_ms = ms;
        self
    }

    /// Get column metadata.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Get all rows.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Get the number of rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Get rows affected (for INSERT, UPDATE, DELETE).
    pub fn rows_affected(&self) -> u64 {
        self.rows_affected
    }

    /// Get execution time in milliseconds.
    pub fn execution_time_ms(&self) -> u64 {
        self.execution_time_ms
    }

    /// Check if the result is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Get the first row.
    pub fn first(&self) -> Option<&Row> {
        self.rows.first()
    }

    /// Get the first value of the first row.
    pub fn scalar(&self) -> Option<&Value> {
        self.first().and_then(|r| r.get(0))
    }

    /// Iterate over rows.
    pub fn iter(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter()
    }
}

impl IntoIterator for QueryResult {
    type Item = Row;
    type IntoIter = std::vec::IntoIter<Row>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.into_iter()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_types() {
        let v = Value::Int(42);
        assert_eq!(v.as_int(), Some(42));
        assert_eq!(v.as_float(), Some(42.0));

        let v = Value::String("hello".to_string());
        assert_eq!(v.as_str(), Some("hello"));

        let v = Value::Null;
        assert!(v.is_null());
    }

    #[test]
    fn test_value_from() {
        let v: Value = 42i32.into();
        assert_eq!(v, Value::Int(42));

        let v: Value = "hello".into();
        assert_eq!(v, Value::String("hello".to_string()));

        let v: Value = true.into();
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn test_row() {
        let columns = vec!["id".to_string(), "name".to_string()];
        let values = vec![Value::Int(1), Value::String("Alice".to_string())];
        let row = Row::new(columns, values);

        assert_eq!(row.len(), 2);
        assert_eq!(row.get_int(0), Some(1));
        assert_eq!(row.get_str(1), Some("Alice"));
        assert_eq!(
            row.get_by_name("name"),
            Some(&Value::String("Alice".to_string()))
        );
    }

    #[test]
    fn test_row_to_map() {
        let columns = vec!["a".to_string(), "b".to_string()];
        let values = vec![Value::Int(1), Value::Int(2)];
        let row = Row::new(columns, values);

        let map = row.to_map();
        assert_eq!(map.get("a"), Some(&Value::Int(1)));
        assert_eq!(map.get("b"), Some(&Value::Int(2)));
    }

    #[test]
    fn test_query_result() {
        let columns = vec![Column::new("id", DataType::Integer)];
        let rows = vec![
            Row::new(vec!["id".to_string()], vec![Value::Int(1)]),
            Row::new(vec!["id".to_string()], vec![Value::Int(2)]),
        ];

        let result = QueryResult::new(columns, rows);
        assert_eq!(result.row_count(), 2);
        assert_eq!(result.scalar(), Some(&Value::Int(1)));
    }

    #[test]
    fn test_query_result_affected() {
        let result = QueryResult::affected(5);
        assert_eq!(result.rows_affected(), 5);
        assert!(result.is_empty());
    }
}
