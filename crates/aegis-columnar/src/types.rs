//! Core types for the columnar / OLAP engine.

use serde::{Deserialize, Serialize};

/// The declared type of a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnType {
    Int,
    Float,
    Text,
    Bool,
}

/// A single cell value. `Null` represents a missing value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(String),
    Null,
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Numeric view of the value (`Int`/`Float`/`Bool`), if any.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// A stable string key for grouping (distinguishes types and null).
    pub fn group_key(&self) -> String {
        match self {
            Value::Int(i) => format!("i:{i}"),
            Value::Float(f) => format!("f:{f}"),
            Value::Bool(b) => format!("b:{b}"),
            Value::Text(s) => format!("s:{s}"),
            Value::Null => "n:".to_string(),
        }
    }

    /// Coerce a JSON value to a typed column value. `null`/absent → `Null`.
    pub fn coerce(json: &serde_json::Value, ty: ColumnType) -> Result<Value, ColumnarError> {
        if json.is_null() {
            return Ok(Value::Null);
        }
        match ty {
            ColumnType::Int => json
                .as_i64()
                .map(Value::Int)
                .or_else(|| json.as_f64().map(|f| Value::Int(f as i64)))
                .ok_or(ColumnarError::TypeMismatch),
            ColumnType::Float => json
                .as_f64()
                .map(Value::Float)
                .ok_or(ColumnarError::TypeMismatch),
            ColumnType::Bool => json
                .as_bool()
                .map(Value::Bool)
                .ok_or(ColumnarError::TypeMismatch),
            ColumnType::Text => json
                .as_str()
                .map(|s| Value::Text(s.to_string()))
                .ok_or(ColumnarError::TypeMismatch),
        }
    }
}

/// A comparison operator used in scan/aggregation predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

/// A single filter condition: `column <op> value`. Conditions are ANDed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub column: String,
    pub op: CompareOp,
    pub value: serde_json::Value,
}

/// An aggregate function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AggFunc {
    Count,
    Sum,
    Min,
    Max,
    Avg,
}

/// An aggregate to compute: `func(column)`. For `count`, `column` may be `"*"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggSpec {
    pub func: AggFunc,
    pub column: String,
}

/// One row of an aggregation result: the group-by key values plus one aggregate
/// value per requested aggregate (in request order).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupRow {
    pub keys: Vec<Value>,
    pub values: Vec<Value>,
}

/// A column definition (name + type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: ColumnType,
}

/// Errors returned by the columnar engine.
#[derive(Debug, thiserror::Error)]
pub enum ColumnarError {
    #[error("table '{0}' not found")]
    TableNotFound(String),
    #[error("table '{0}' already exists")]
    TableExists(String),
    #[error("unknown column '{0}'")]
    UnknownColumn(String),
    #[error("value does not match the column type")]
    TypeMismatch,
    #[error("table must declare at least one column")]
    EmptySchema,
    #[error("duplicate column '{0}' in schema")]
    DuplicateColumn(String),
}
