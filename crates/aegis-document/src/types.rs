//! Aegis Document Types
//!
//! Core data types for document storage.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fmt;

// =============================================================================
// Document ID
// =============================================================================

/// Unique identifier for a document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub String);

impl DocumentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn generate() -> Self {
        Self(uuid())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for DocumentId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for DocumentId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = duration.as_nanos();
    let random: u64 = (nanos as u64).wrapping_mul(0x5851_f42d_4c95_7f2d);
    format!("{:016x}{:016x}", nanos as u64, random)
}

// =============================================================================
// Value
// =============================================================================

/// A document value that can be any JSON-compatible type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[derive(Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Self::Int(_) | Self::Float(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            Self::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Self::Array(arr) => Some(arr),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Self::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// Get a value at a path (e.g., "user.address.city").
    pub fn get_path(&self, path: &str) -> Option<&Value> {
        let parts: Vec<&str> = path.split('.').collect();
        self.get_path_parts(&parts)
    }

    fn get_path_parts(&self, parts: &[&str]) -> Option<&Value> {
        if parts.is_empty() {
            return Some(self);
        }

        let key = parts[0];
        let rest = &parts[1..];

        match self {
            Self::Object(obj) => obj.get(key).and_then(|v| v.get_path_parts(rest)),
            Self::Array(arr) => {
                key.parse::<usize>()
                    .ok()
                    .and_then(|idx| arr.get(idx))
                    .and_then(|v| v.get_path_parts(rest))
            }
            _ => None,
        }
    }

    /// Convert from serde_json::Value.
    pub fn from_json(json: JsonValue) -> Self {
        match json {
            JsonValue::Null => Self::Null,
            JsonValue::Bool(b) => Self::Bool(b),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Self::Int(i)
                } else if let Some(f) = n.as_f64() {
                    Self::Float(f)
                } else {
                    Self::Float(0.0)
                }
            }
            JsonValue::String(s) => Self::String(s),
            JsonValue::Array(arr) => Self::Array(arr.into_iter().map(Self::from_json).collect()),
            JsonValue::Object(obj) => {
                Self::Object(obj.into_iter().map(|(k, v)| (k, Self::from_json(v))).collect())
            }
        }
    }

    /// Convert to serde_json::Value.
    pub fn to_json(&self) -> JsonValue {
        match self {
            Self::Null => JsonValue::Null,
            Self::Bool(b) => JsonValue::Bool(*b),
            Self::Int(n) => JsonValue::Number((*n).into()),
            Self::Float(f) => {
                serde_json::Number::from_f64(*f)
                    .map(JsonValue::Number)
                    .unwrap_or(JsonValue::Null)
            }
            Self::String(s) => JsonValue::String(s.clone()),
            Self::Array(arr) => JsonValue::Array(arr.iter().map(|v| v.to_json()).collect()),
            Self::Object(obj) => {
                JsonValue::Object(obj.iter().map(|(k, v)| (k.clone(), v.to_json())).collect())
            }
        }
    }
}


impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Int(n)
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Self::Int(n as i64)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<Vec<Value>> for Value {
    fn from(arr: Vec<Value>) -> Self {
        Self::Array(arr)
    }
}

impl From<HashMap<String, Value>> for Value {
    fn from(obj: HashMap<String, Value>) -> Self {
        Self::Object(obj)
    }
}

// =============================================================================
// Document
// =============================================================================

/// A document in the document store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    #[serde(rename = "_id")]
    pub id: DocumentId,
    #[serde(flatten)]
    pub data: HashMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _updated_at: Option<i64>,
}

impl Document {
    /// Create a new document with an auto-generated ID.
    pub fn new() -> Self {
        Self {
            id: DocumentId::generate(),
            data: HashMap::new(),
            _created_at: Some(current_timestamp()),
            _updated_at: None,
        }
    }

    /// Create a document with a specific ID.
    pub fn with_id(id: impl Into<DocumentId>) -> Self {
        Self {
            id: id.into(),
            data: HashMap::new(),
            _created_at: Some(current_timestamp()),
            _updated_at: None,
        }
    }

    /// Create a document from JSON.
    pub fn from_json(json: JsonValue) -> Option<Self> {
        match json {
            JsonValue::Object(obj) => {
                let id = obj
                    .get("_id")
                    .and_then(|v| v.as_str())
                    .map(DocumentId::new)
                    .unwrap_or_else(DocumentId::generate);

                let data: HashMap<String, Value> = obj
                    .into_iter()
                    .filter(|(k, _)| !k.starts_with('_'))
                    .map(|(k, v)| (k, Value::from_json(v)))
                    .collect();

                Some(Self {
                    id,
                    data,
                    _created_at: Some(current_timestamp()),
                    _updated_at: None,
                })
            }
            _ => None,
        }
    }

    /// Convert to JSON.
    pub fn to_json(&self) -> JsonValue {
        let mut obj = serde_json::Map::new();
        obj.insert("_id".to_string(), JsonValue::String(self.id.0.clone()));

        if let Some(ts) = self._created_at {
            obj.insert("_created_at".to_string(), JsonValue::Number(ts.into()));
        }
        if let Some(ts) = self._updated_at {
            obj.insert("_updated_at".to_string(), JsonValue::Number(ts.into()));
        }

        for (k, v) in &self.data {
            obj.insert(k.clone(), v.to_json());
        }

        JsonValue::Object(obj)
    }

    /// Get a field value.
    pub fn get(&self, key: &str) -> Option<&Value> {
        if key.contains('.') {
            let parts: Vec<&str> = key.splitn(2, '.').collect();
            self.data
                .get(parts[0])
                .and_then(|v| v.get_path(parts[1]))
        } else {
            self.data.get(key)
        }
    }

    /// Set a field value.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        self.data.insert(key.into(), value.into());
        self._updated_at = Some(current_timestamp());
    }

    /// Remove a field.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let result = self.data.remove(key);
        if result.is_some() {
            self._updated_at = Some(current_timestamp());
        }
        result
    }

    /// Check if a field exists.
    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Get all field names.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.data.keys()
    }

    /// Get the number of fields.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the document is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

fn current_timestamp() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_id() {
        let id1 = DocumentId::generate();
        let id2 = DocumentId::generate();
        assert_ne!(id1, id2);

        let id3 = DocumentId::new("custom-id");
        assert_eq!(id3.as_str(), "custom-id");
    }

    #[test]
    fn test_value_types() {
        let null = Value::Null;
        assert!(null.is_null());

        let boolean = Value::Bool(true);
        assert!(boolean.is_bool());
        assert_eq!(boolean.as_bool(), Some(true));

        let number = Value::Int(42);
        assert!(number.is_number());
        assert_eq!(number.as_i64(), Some(42));

        let string = Value::String("hello".to_string());
        assert!(string.is_string());
        assert_eq!(string.as_str(), Some("hello"));
    }

    #[test]
    fn test_value_path() {
        let mut inner = HashMap::new();
        inner.insert("city".to_string(), Value::String("NYC".to_string()));

        let mut outer = HashMap::new();
        outer.insert("address".to_string(), Value::Object(inner));

        let value = Value::Object(outer);

        assert_eq!(
            value.get_path("address.city").and_then(|v| v.as_str()),
            Some("NYC")
        );
    }

    #[test]
    fn test_document() {
        let mut doc = Document::new();
        doc.set("name", "Alice");
        doc.set("age", 30i64);

        assert_eq!(doc.get("name").and_then(|v| v.as_str()), Some("Alice"));
        assert_eq!(doc.get("age").and_then(|v| v.as_i64()), Some(30));

        assert!(doc.contains("name"));
        assert!(!doc.contains("email"));
    }

    #[test]
    fn test_document_from_json() {
        let json = serde_json::json!({
            "_id": "doc123",
            "name": "Bob",
            "active": true
        });

        let doc = Document::from_json(json).unwrap();
        assert_eq!(doc.id.as_str(), "doc123");
        assert_eq!(doc.get("name").and_then(|v| v.as_str()), Some("Bob"));
        assert_eq!(doc.get("active").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn test_json_conversion() {
        let mut doc = Document::with_id("test-doc");
        doc.set("count", 100i64);
        doc.set("ratio", 0.5f64);

        let json = doc.to_json();
        assert_eq!(json["_id"], "test-doc");
        assert_eq!(json["count"], 100);
    }
}
