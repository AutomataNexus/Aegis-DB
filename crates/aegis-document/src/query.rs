//! Aegis Document Query
//!
//! Query language for document filtering and retrieval.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::types::{Document, Value};
use serde::{Deserialize, Serialize};

// =============================================================================
// Query
// =============================================================================

/// A query for filtering documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub filters: Vec<Filter>,
    pub sort: Option<Sort>,
    pub skip: Option<usize>,
    pub limit: Option<usize>,
    pub projection: Option<Vec<String>>,
}

impl Query {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
            sort: None,
            skip: None,
            limit: None,
            projection: None,
        }
    }

    /// Check if a document matches this query.
    pub fn matches(&self, doc: &Document) -> bool {
        self.filters.iter().all(|f| f.matches(doc))
    }

    /// Add a filter.
    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filters.push(filter);
        self
    }

    /// Add sorting.
    pub fn with_sort(mut self, field: impl Into<String>, ascending: bool) -> Self {
        self.sort = Some(Sort {
            field: field.into(),
            ascending,
        });
        self
    }

    /// Add skip.
    pub fn with_skip(mut self, skip: usize) -> Self {
        self.skip = Some(skip);
        self
    }

    /// Add limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Add projection.
    pub fn with_projection(mut self, fields: Vec<String>) -> Self {
        self.projection = Some(fields);
        self
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Filter
// =============================================================================

/// A filter condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Filter {
    Eq { field: String, value: Value },
    Ne { field: String, value: Value },
    Gt { field: String, value: Value },
    Gte { field: String, value: Value },
    Lt { field: String, value: Value },
    Lte { field: String, value: Value },
    In { field: String, values: Vec<Value> },
    Nin { field: String, values: Vec<Value> },
    Exists { field: String, exists: bool },
    Regex { field: String, pattern: String },
    Contains { field: String, value: String },
    StartsWith { field: String, value: String },
    EndsWith { field: String, value: String },
    And(Vec<Filter>),
    Or(Vec<Filter>),
    Not(Box<Filter>),
}

impl Filter {
    /// Check if a document matches this filter.
    pub fn matches(&self, doc: &Document) -> bool {
        match self {
            Self::Eq { field, value } => {
                doc.get(field).map(|v| v == value).unwrap_or(false)
            }
            Self::Ne { field, value } => {
                doc.get(field).map(|v| v != value).unwrap_or(true)
            }
            Self::Gt { field, value } => {
                doc.get(field)
                    .map(|v| compare_values(v, value) == Some(std::cmp::Ordering::Greater))
                    .unwrap_or(false)
            }
            Self::Gte { field, value } => {
                doc.get(field)
                    .map(|v| {
                        matches!(
                            compare_values(v, value),
                            Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                        )
                    })
                    .unwrap_or(false)
            }
            Self::Lt { field, value } => {
                doc.get(field)
                    .map(|v| compare_values(v, value) == Some(std::cmp::Ordering::Less))
                    .unwrap_or(false)
            }
            Self::Lte { field, value } => {
                doc.get(field)
                    .map(|v| {
                        matches!(
                            compare_values(v, value),
                            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                        )
                    })
                    .unwrap_or(false)
            }
            Self::In { field, values } => {
                doc.get(field)
                    .map(|v| values.contains(v))
                    .unwrap_or(false)
            }
            Self::Nin { field, values } => {
                doc.get(field)
                    .map(|v| !values.contains(v))
                    .unwrap_or(true)
            }
            Self::Exists { field, exists } => doc.contains(field) == *exists,
            Self::Regex { field, pattern } => {
                if let Some(Value::String(s)) = doc.get(field) {
                    regex::Regex::new(pattern)
                        .map(|re| re.is_match(s))
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            Self::Contains { field, value } => {
                if let Some(Value::String(s)) = doc.get(field) {
                    s.contains(value)
                } else {
                    false
                }
            }
            Self::StartsWith { field, value } => {
                if let Some(Value::String(s)) = doc.get(field) {
                    s.starts_with(value)
                } else {
                    false
                }
            }
            Self::EndsWith { field, value } => {
                if let Some(Value::String(s)) = doc.get(field) {
                    s.ends_with(value)
                } else {
                    false
                }
            }
            Self::And(filters) => filters.iter().all(|f| f.matches(doc)),
            Self::Or(filters) => filters.iter().any(|f| f.matches(doc)),
            Self::Not(filter) => !filter.matches(doc),
        }
    }
}

fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
        (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
        _ => None,
    }
}

// =============================================================================
// Sort
// =============================================================================

/// Sort specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sort {
    pub field: String,
    pub ascending: bool,
}

// =============================================================================
// Query Builder
// =============================================================================

/// Builder for constructing queries.
pub struct QueryBuilder {
    query: Query,
}

impl QueryBuilder {
    pub fn new() -> Self {
        Self {
            query: Query::new(),
        }
    }

    pub fn eq(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.query.filters.push(Filter::Eq {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn ne(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.query.filters.push(Filter::Ne {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn gt(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.query.filters.push(Filter::Gt {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn gte(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.query.filters.push(Filter::Gte {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn lt(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.query.filters.push(Filter::Lt {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn lte(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
        self.query.filters.push(Filter::Lte {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn in_values(mut self, field: impl Into<String>, values: Vec<Value>) -> Self {
        self.query.filters.push(Filter::In {
            field: field.into(),
            values,
        });
        self
    }

    pub fn exists(mut self, field: impl Into<String>, exists: bool) -> Self {
        self.query.filters.push(Filter::Exists {
            field: field.into(),
            exists,
        });
        self
    }

    pub fn contains(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.filters.push(Filter::Contains {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn starts_with(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.filters.push(Filter::StartsWith {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn ends_with(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.filters.push(Filter::EndsWith {
            field: field.into(),
            value: value.into(),
        });
        self
    }

    pub fn regex(mut self, field: impl Into<String>, pattern: impl Into<String>) -> Self {
        self.query.filters.push(Filter::Regex {
            field: field.into(),
            pattern: pattern.into(),
        });
        self
    }

    pub fn and(mut self, filters: Vec<Filter>) -> Self {
        self.query.filters.push(Filter::And(filters));
        self
    }

    pub fn or(mut self, filters: Vec<Filter>) -> Self {
        self.query.filters.push(Filter::Or(filters));
        self
    }

    pub fn sort(mut self, field: impl Into<String>, ascending: bool) -> Self {
        self.query.sort = Some(Sort {
            field: field.into(),
            ascending,
        });
        self
    }

    pub fn skip(mut self, skip: usize) -> Self {
        self.query.skip = Some(skip);
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.query.limit = Some(limit);
        self
    }

    pub fn project(mut self, fields: Vec<String>) -> Self {
        self.query.projection = Some(fields);
        self
    }

    pub fn build(self) -> Query {
        self.query
    }
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Query Result
// =============================================================================

/// Result of a document query.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub documents: Vec<Document>,
    pub total_scanned: usize,
    pub execution_time_ms: u64,
}

impl QueryResult {
    pub fn empty() -> Self {
        Self {
            documents: Vec::new(),
            total_scanned: 0,
            execution_time_ms: 0,
        }
    }

    pub fn count(&self) -> usize {
        self.documents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    pub fn first(&self) -> Option<&Document> {
        self.documents.first()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_doc() -> Document {
        let mut doc = Document::with_id("test");
        doc.set("name", "Alice");
        doc.set("age", 30i64);
        doc.set("active", true);
        doc.set("email", "alice@example.com");
        doc
    }

    #[test]
    fn test_eq_filter() {
        let doc = create_test_doc();

        let filter = Filter::Eq {
            field: "name".to_string(),
            value: Value::String("Alice".to_string()),
        };
        assert!(filter.matches(&doc));

        let filter = Filter::Eq {
            field: "name".to_string(),
            value: Value::String("Bob".to_string()),
        };
        assert!(!filter.matches(&doc));
    }

    #[test]
    fn test_comparison_filters() {
        let doc = create_test_doc();

        let filter = Filter::Gt {
            field: "age".to_string(),
            value: Value::Int(25),
        };
        assert!(filter.matches(&doc));

        let filter = Filter::Lt {
            field: "age".to_string(),
            value: Value::Int(25),
        };
        assert!(!filter.matches(&doc));

        let filter = Filter::Gte {
            field: "age".to_string(),
            value: Value::Int(30),
        };
        assert!(filter.matches(&doc));
    }

    #[test]
    fn test_string_filters() {
        let doc = create_test_doc();

        let filter = Filter::Contains {
            field: "email".to_string(),
            value: "example".to_string(),
        };
        assert!(filter.matches(&doc));

        let filter = Filter::StartsWith {
            field: "email".to_string(),
            value: "alice".to_string(),
        };
        assert!(filter.matches(&doc));

        let filter = Filter::EndsWith {
            field: "email".to_string(),
            value: ".com".to_string(),
        };
        assert!(filter.matches(&doc));
    }

    #[test]
    fn test_logical_filters() {
        let doc = create_test_doc();

        let filter = Filter::And(vec![
            Filter::Eq {
                field: "name".to_string(),
                value: Value::String("Alice".to_string()),
            },
            Filter::Gt {
                field: "age".to_string(),
                value: Value::Int(20),
            },
        ]);
        assert!(filter.matches(&doc));

        let filter = Filter::Or(vec![
            Filter::Eq {
                field: "name".to_string(),
                value: Value::String("Bob".to_string()),
            },
            Filter::Eq {
                field: "active".to_string(),
                value: Value::Bool(true),
            },
        ]);
        assert!(filter.matches(&doc));
    }

    #[test]
    fn test_query_builder() {
        let doc = create_test_doc();

        let query = QueryBuilder::new()
            .eq("name", "Alice")
            .gt("age", 25i64)
            .build();

        assert!(query.matches(&doc));

        let query = QueryBuilder::new()
            .eq("name", "Bob")
            .build();

        assert!(!query.matches(&doc));
    }

    #[test]
    fn test_exists_filter() {
        let doc = create_test_doc();

        let filter = Filter::Exists {
            field: "name".to_string(),
            exists: true,
        };
        assert!(filter.matches(&doc));

        let filter = Filter::Exists {
            field: "missing".to_string(),
            exists: false,
        };
        assert!(filter.matches(&doc));
    }
}
