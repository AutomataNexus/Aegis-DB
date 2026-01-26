//! Aegis Document Validation
//!
//! Schema validation for documents.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::types::{Document, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Schema
// =============================================================================

/// Schema definition for document validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub fields: HashMap<String, FieldSchema>,
    pub required: Vec<String>,
    pub additional_properties: bool,
}

impl Schema {
    /// Create a new schema.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fields: HashMap::new(),
            required: Vec::new(),
            additional_properties: true,
        }
    }

    /// Add a field to the schema.
    pub fn field(mut self, name: impl Into<String>, schema: FieldSchema) -> Self {
        self.fields.insert(name.into(), schema);
        self
    }

    /// Add a required field.
    pub fn require(mut self, name: impl Into<String>) -> Self {
        self.required.push(name.into());
        self
    }

    /// Set whether additional properties are allowed.
    pub fn additional_properties(mut self, allow: bool) -> Self {
        self.additional_properties = allow;
        self
    }

    /// Validate a document against this schema.
    pub fn validate(&self, doc: &Document) -> ValidationResult {
        let mut errors = Vec::new();

        for required in &self.required {
            if !doc.contains(required) {
                errors.push(format!("Missing required field: {}", required));
            }
        }

        for (field_name, field_schema) in &self.fields {
            if let Some(value) = doc.get(field_name) {
                if let Err(err) = field_schema.validate(value) {
                    errors.push(format!("Field '{}': {}", field_name, err));
                }
            }
        }

        if !self.additional_properties {
            for key in doc.keys() {
                if !self.fields.contains_key(key) {
                    errors.push(format!("Unknown field: {}", key));
                }
            }
        }

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
        }
    }
}

// =============================================================================
// Field Schema
// =============================================================================

/// Schema for a single field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSchema {
    pub field_type: FieldType,
    pub nullable: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
    pub enum_values: Option<Vec<Value>>,
    pub items: Option<Box<FieldSchema>>,
    pub properties: Option<HashMap<String, FieldSchema>>,
}

impl FieldSchema {
    pub fn new(field_type: FieldType) -> Self {
        Self {
            field_type,
            nullable: false,
            min: None,
            max: None,
            min_length: None,
            max_length: None,
            pattern: None,
            enum_values: None,
            items: None,
            properties: None,
        }
    }

    pub fn string() -> Self {
        Self::new(FieldType::String)
    }

    pub fn int() -> Self {
        Self::new(FieldType::Int)
    }

    pub fn float() -> Self {
        Self::new(FieldType::Float)
    }

    pub fn bool() -> Self {
        Self::new(FieldType::Bool)
    }

    pub fn array(items: FieldSchema) -> Self {
        let mut schema = Self::new(FieldType::Array);
        schema.items = Some(Box::new(items));
        schema
    }

    pub fn object() -> Self {
        Self::new(FieldType::Object)
    }

    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }

    pub fn min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    pub fn min_length(mut self, len: usize) -> Self {
        self.min_length = Some(len);
        self
    }

    pub fn max_length(mut self, len: usize) -> Self {
        self.max_length = Some(len);
        self
    }

    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    pub fn enum_values(mut self, values: Vec<Value>) -> Self {
        self.enum_values = Some(values);
        self
    }

    /// Validate a value against this field schema.
    pub fn validate(&self, value: &Value) -> Result<(), String> {
        if value.is_null() {
            if self.nullable {
                return Ok(());
            }
            return Err("Value cannot be null".to_string());
        }

        if !self.field_type.matches(value) {
            return Err(format!(
                "Expected type {:?}, got {:?}",
                self.field_type,
                value_type(value)
            ));
        }

        if let Some(ref enum_values) = self.enum_values {
            if !enum_values.contains(value) {
                return Err("Value not in allowed enum values".to_string());
            }
        }

        match value {
            Value::Int(n) => {
                if let Some(min) = self.min {
                    if (*n as f64) < min {
                        return Err(format!("Value {} is less than minimum {}", n, min));
                    }
                }
                if let Some(max) = self.max {
                    if (*n as f64) > max {
                        return Err(format!("Value {} is greater than maximum {}", n, max));
                    }
                }
            }
            Value::Float(f) => {
                if let Some(min) = self.min {
                    if *f < min {
                        return Err(format!("Value {} is less than minimum {}", f, min));
                    }
                }
                if let Some(max) = self.max {
                    if *f > max {
                        return Err(format!("Value {} is greater than maximum {}", f, max));
                    }
                }
            }
            Value::String(s) => {
                if let Some(min_len) = self.min_length {
                    if s.len() < min_len {
                        return Err(format!(
                            "String length {} is less than minimum {}",
                            s.len(),
                            min_len
                        ));
                    }
                }
                if let Some(max_len) = self.max_length {
                    if s.len() > max_len {
                        return Err(format!(
                            "String length {} is greater than maximum {}",
                            s.len(),
                            max_len
                        ));
                    }
                }
                if let Some(ref pattern) = self.pattern {
                    // Use RegexBuilder with size_limit to prevent ReDoS attacks
                    // from catastrophic backtracking on malicious patterns.
                    // 1MB compiled size limit is sufficient for legitimate patterns
                    // while blocking pathological cases.
                    let re = regex::RegexBuilder::new(pattern)
                        .size_limit(1024 * 1024) // 1MB compiled size limit
                        .build()
                        .map_err(|e| format!("Invalid regex pattern: {}", e))?;
                    if !re.is_match(s) {
                        return Err(format!("String does not match pattern: {}", pattern));
                    }
                }
            }
            Value::Array(arr) => {
                if let Some(min_len) = self.min_length {
                    if arr.len() < min_len {
                        return Err(format!(
                            "Array length {} is less than minimum {}",
                            arr.len(),
                            min_len
                        ));
                    }
                }
                if let Some(max_len) = self.max_length {
                    if arr.len() > max_len {
                        return Err(format!(
                            "Array length {} is greater than maximum {}",
                            arr.len(),
                            max_len
                        ));
                    }
                }
                if let Some(ref items_schema) = self.items {
                    for (i, item) in arr.iter().enumerate() {
                        if let Err(e) = items_schema.validate(item) {
                            return Err(format!("Array item {}: {}", i, e));
                        }
                    }
                }
            }
            Value::Object(obj) => {
                if let Some(ref props) = self.properties {
                    for (key, prop_schema) in props {
                        if let Some(value) = obj.get(key) {
                            if let Err(e) = prop_schema.validate(value) {
                                return Err(format!("Property '{}': {}", key, e));
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

// =============================================================================
// Field Type
// =============================================================================

/// Type of a field value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    String,
    Int,
    Float,
    Number,
    Bool,
    Array,
    Object,
    Any,
}

impl FieldType {
    fn matches(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::Any, _) => true,
            (Self::String, Value::String(_)) => true,
            (Self::Int, Value::Int(_)) => true,
            (Self::Float, Value::Float(_)) => true,
            (Self::Number, Value::Int(_) | Value::Float(_)) => true,
            (Self::Bool, Value::Bool(_)) => true,
            (Self::Array, Value::Array(_)) => true,
            (Self::Object, Value::Object(_)) => true,
            _ => false,
        }
    }
}

// =============================================================================
// Validation Result
// =============================================================================

/// Result of schema validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
        }
    }

    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            errors,
        }
    }
}

// =============================================================================
// Schema Builder
// =============================================================================

/// Builder for creating schemas.
pub struct SchemaBuilder {
    schema: Schema,
}

impl SchemaBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema: Schema::new(name),
        }
    }

    pub fn field(mut self, name: impl Into<String>, schema: FieldSchema) -> Self {
        self.schema.fields.insert(name.into(), schema);
        self
    }

    pub fn required_field(mut self, name: impl Into<String>, schema: FieldSchema) -> Self {
        let name = name.into();
        self.schema.fields.insert(name.clone(), schema);
        self.schema.required.push(name);
        self
    }

    pub fn additional_properties(mut self, allow: bool) -> Self {
        self.schema.additional_properties = allow;
        self
    }

    pub fn build(self) -> Schema {
        self.schema
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_validation() {
        let schema = FieldSchema::string();
        assert!(schema.validate(&Value::String("hello".to_string())).is_ok());
        assert!(schema.validate(&Value::Int(42)).is_err());

        let schema = FieldSchema::int();
        assert!(schema.validate(&Value::Int(42)).is_ok());
        assert!(schema.validate(&Value::String("42".to_string())).is_err());
    }

    #[test]
    fn test_nullable() {
        let schema = FieldSchema::string();
        assert!(schema.validate(&Value::Null).is_err());

        let schema = FieldSchema::string().nullable();
        assert!(schema.validate(&Value::Null).is_ok());
    }

    #[test]
    fn test_range_validation() {
        let schema = FieldSchema::int().min(0.0).max(100.0);

        assert!(schema.validate(&Value::Int(50)).is_ok());
        assert!(schema.validate(&Value::Int(-1)).is_err());
        assert!(schema.validate(&Value::Int(101)).is_err());
    }

    #[test]
    fn test_string_length() {
        let schema = FieldSchema::string().min_length(3).max_length(10);

        assert!(schema.validate(&Value::String("hello".to_string())).is_ok());
        assert!(schema.validate(&Value::String("hi".to_string())).is_err());
        assert!(schema
            .validate(&Value::String("hello world!".to_string()))
            .is_err());
    }

    #[test]
    fn test_pattern_validation() {
        let schema = FieldSchema::string().pattern(r"^\d{3}-\d{4}$");

        assert!(schema.validate(&Value::String("123-4567".to_string())).is_ok());
        assert!(schema.validate(&Value::String("invalid".to_string())).is_err());
    }

    #[test]
    fn test_schema_validation() {
        let schema = SchemaBuilder::new("User")
            .required_field("name", FieldSchema::string().min_length(1))
            .required_field("age", FieldSchema::int().min(0.0))
            .field("email", FieldSchema::string().nullable())
            .build();

        let mut doc = Document::new();
        doc.set("name", "Alice");
        doc.set("age", 30i64);

        let result = schema.validate(&doc);
        assert!(result.is_valid);

        let mut invalid_doc = Document::new();
        invalid_doc.set("name", "Bob");

        let result = schema.validate(&invalid_doc);
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("age")));
    }

    #[test]
    fn test_enum_validation() {
        let schema = FieldSchema::string().enum_values(vec![
            Value::String("active".to_string()),
            Value::String("inactive".to_string()),
        ]);

        assert!(schema.validate(&Value::String("active".to_string())).is_ok());
        assert!(schema.validate(&Value::String("unknown".to_string())).is_err());
    }

    #[test]
    fn test_array_validation() {
        let schema = FieldSchema::array(FieldSchema::int()).min_length(1).max_length(5);

        assert!(schema.validate(&Value::Array(vec![Value::Int(1), Value::Int(2)])).is_ok());
        assert!(schema.validate(&Value::Array(vec![])).is_err());
        assert!(schema
            .validate(&Value::Array(vec![Value::String("not an int".to_string())]))
            .is_err());
    }
}
