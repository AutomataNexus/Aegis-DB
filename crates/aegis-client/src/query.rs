//! Aegis Client Query Builder
//!
//! Type-safe query building for Aegis database.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::result::Value;
use serde::{Deserialize, Serialize};

// =============================================================================
// Query
// =============================================================================

/// A prepared query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub sql: String,
    pub params: Vec<Value>,
}

impl Query {
    /// Create a new query.
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            params: Vec::new(),
        }
    }

    /// Add a parameter.
    pub fn param(mut self, value: impl Into<Value>) -> Self {
        self.params.push(value.into());
        self
    }

    /// Add multiple parameters.
    pub fn params(mut self, values: Vec<Value>) -> Self {
        self.params.extend(values);
        self
    }
}

// =============================================================================
// Query Builder
// =============================================================================

/// Builder for SQL queries.
#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
    operation: QueryOperation,
    table: Option<String>,
    columns: Vec<String>,
    values: Vec<Vec<Value>>,
    conditions: Vec<Condition>,
    order_by: Vec<OrderBy>,
    limit: Option<u64>,
    offset: Option<u64>,
    joins: Vec<Join>,
    group_by: Vec<String>,
    having: Vec<Condition>,
}

#[derive(Debug, Clone, Default)]
enum QueryOperation {
    #[default]
    Select,
    Insert,
    Update,
    Delete,
}

impl QueryBuilder {
    /// Create a new query builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a SELECT query.
    pub fn select(mut self, columns: &[&str]) -> Self {
        self.operation = QueryOperation::Select;
        self.columns = columns.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Start an INSERT query.
    pub fn insert_into(mut self, table: &str) -> Self {
        self.operation = QueryOperation::Insert;
        self.table = Some(table.to_string());
        self
    }

    /// Start an UPDATE query.
    pub fn update(mut self, table: &str) -> Self {
        self.operation = QueryOperation::Update;
        self.table = Some(table.to_string());
        self
    }

    /// Start a DELETE query.
    pub fn delete_from(mut self, table: &str) -> Self {
        self.operation = QueryOperation::Delete;
        self.table = Some(table.to_string());
        self
    }

    /// Set the table for SELECT.
    pub fn from(mut self, table: &str) -> Self {
        self.table = Some(table.to_string());
        self
    }

    /// Set columns for INSERT.
    pub fn columns(mut self, columns: &[&str]) -> Self {
        self.columns = columns.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Add values for INSERT.
    pub fn values(mut self, values: Vec<Value>) -> Self {
        self.values.push(values);
        self
    }

    /// Add a WHERE condition.
    pub fn where_eq(mut self, column: &str, value: impl Into<Value>) -> Self {
        self.conditions.push(Condition::Eq(column.to_string(), value.into()));
        self
    }

    /// Add a WHERE != condition.
    pub fn where_ne(mut self, column: &str, value: impl Into<Value>) -> Self {
        self.conditions.push(Condition::Ne(column.to_string(), value.into()));
        self
    }

    /// Add a WHERE > condition.
    pub fn where_gt(mut self, column: &str, value: impl Into<Value>) -> Self {
        self.conditions.push(Condition::Gt(column.to_string(), value.into()));
        self
    }

    /// Add a WHERE >= condition.
    pub fn where_gte(mut self, column: &str, value: impl Into<Value>) -> Self {
        self.conditions.push(Condition::Gte(column.to_string(), value.into()));
        self
    }

    /// Add a WHERE < condition.
    pub fn where_lt(mut self, column: &str, value: impl Into<Value>) -> Self {
        self.conditions.push(Condition::Lt(column.to_string(), value.into()));
        self
    }

    /// Add a WHERE <= condition.
    pub fn where_lte(mut self, column: &str, value: impl Into<Value>) -> Self {
        self.conditions.push(Condition::Lte(column.to_string(), value.into()));
        self
    }

    /// Add a WHERE LIKE condition.
    pub fn where_like(mut self, column: &str, pattern: &str) -> Self {
        self.conditions.push(Condition::Like(column.to_string(), pattern.to_string()));
        self
    }

    /// Add a WHERE IN condition.
    pub fn where_in(mut self, column: &str, values: Vec<Value>) -> Self {
        self.conditions.push(Condition::In(column.to_string(), values));
        self
    }

    /// Add a WHERE IS NULL condition.
    pub fn where_null(mut self, column: &str) -> Self {
        self.conditions.push(Condition::IsNull(column.to_string()));
        self
    }

    /// Add a WHERE IS NOT NULL condition.
    pub fn where_not_null(mut self, column: &str) -> Self {
        self.conditions.push(Condition::IsNotNull(column.to_string()));
        self
    }

    /// Add a JOIN clause.
    pub fn join(mut self, table: &str, on: &str) -> Self {
        self.joins.push(Join {
            join_type: JoinType::Inner,
            table: table.to_string(),
            on: on.to_string(),
        });
        self
    }

    /// Add a LEFT JOIN clause.
    pub fn left_join(mut self, table: &str, on: &str) -> Self {
        self.joins.push(Join {
            join_type: JoinType::Left,
            table: table.to_string(),
            on: on.to_string(),
        });
        self
    }

    /// Add an ORDER BY clause.
    pub fn order_by(mut self, column: &str, direction: OrderDirection) -> Self {
        self.order_by.push(OrderBy {
            column: column.to_string(),
            direction,
        });
        self
    }

    /// Add ascending ORDER BY.
    pub fn order_by_asc(self, column: &str) -> Self {
        self.order_by(column, OrderDirection::Asc)
    }

    /// Add descending ORDER BY.
    pub fn order_by_desc(self, column: &str) -> Self {
        self.order_by(column, OrderDirection::Desc)
    }

    /// Add a GROUP BY clause.
    pub fn group_by(mut self, columns: &[&str]) -> Self {
        self.group_by = columns.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Add a HAVING condition.
    pub fn having(mut self, condition: Condition) -> Self {
        self.having.push(condition);
        self
    }

    /// Set the LIMIT.
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set the OFFSET.
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Set values for UPDATE.
    pub fn set(mut self, column: &str, value: impl Into<Value>) -> Self {
        self.columns.push(column.to_string());
        if self.values.is_empty() {
            self.values.push(Vec::new());
        }
        self.values[0].push(value.into());
        self
    }

    /// Build the query.
    pub fn build(self) -> Query {
        let (sql, params) = match self.operation {
            QueryOperation::Select => self.build_select(),
            QueryOperation::Insert => self.build_insert(),
            QueryOperation::Update => self.build_update(),
            QueryOperation::Delete => self.build_delete(),
        };

        Query { sql, params }
    }

    fn build_select(&self) -> (String, Vec<Value>) {
        let mut sql = String::from("SELECT ");
        let mut params = Vec::new();

        if self.columns.is_empty() {
            sql.push('*');
        } else {
            sql.push_str(&self.columns.join(", "));
        }

        if let Some(ref table) = self.table {
            sql.push_str(" FROM ");
            sql.push_str(table);
        }

        for join in &self.joins {
            sql.push_str(&format!(
                " {} JOIN {} ON {}",
                join.join_type.as_str(),
                join.table,
                join.on
            ));
        }

        self.append_where(&mut sql, &mut params);
        self.append_group_by(&mut sql);
        self.append_having(&mut sql, &mut params);
        self.append_order_by(&mut sql);
        self.append_limit_offset(&mut sql);

        (sql, params)
    }

    fn build_insert(&self) -> (String, Vec<Value>) {
        let mut sql = String::from("INSERT INTO ");
        let mut params = Vec::new();

        if let Some(ref table) = self.table {
            sql.push_str(table);
        }

        if !self.columns.is_empty() {
            sql.push_str(" (");
            sql.push_str(&self.columns.join(", "));
            sql.push(')');
        }

        sql.push_str(" VALUES ");

        let value_groups: Vec<String> = self
            .values
            .iter()
            .map(|row| {
                let placeholders: Vec<String> = row
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        params.push(v.clone());
                        format!("${}", params.len())
                    })
                    .collect();
                format!("({})", placeholders.join(", "))
            })
            .collect();

        sql.push_str(&value_groups.join(", "));

        (sql, params)
    }

    fn build_update(&self) -> (String, Vec<Value>) {
        let mut sql = String::from("UPDATE ");
        let mut params = Vec::new();

        if let Some(ref table) = self.table {
            sql.push_str(table);
        }

        sql.push_str(" SET ");

        let sets: Vec<String> = self
            .columns
            .iter()
            .zip(self.values.first().unwrap_or(&Vec::new()).iter())
            .map(|(col, val)| {
                params.push(val.clone());
                format!("{} = ${}", col, params.len())
            })
            .collect();

        sql.push_str(&sets.join(", "));

        self.append_where(&mut sql, &mut params);

        (sql, params)
    }

    fn build_delete(&self) -> (String, Vec<Value>) {
        let mut sql = String::from("DELETE FROM ");
        let mut params = Vec::new();

        if let Some(ref table) = self.table {
            sql.push_str(table);
        }

        self.append_where(&mut sql, &mut params);

        (sql, params)
    }

    fn append_where(&self, sql: &mut String, params: &mut Vec<Value>) {
        if self.conditions.is_empty() {
            return;
        }

        sql.push_str(" WHERE ");

        let conditions: Vec<String> = self
            .conditions
            .iter()
            .map(|c| c.to_sql(params))
            .collect();

        sql.push_str(&conditions.join(" AND "));
    }

    fn append_group_by(&self, sql: &mut String) {
        if !self.group_by.is_empty() {
            sql.push_str(" GROUP BY ");
            sql.push_str(&self.group_by.join(", "));
        }
    }

    fn append_having(&self, sql: &mut String, params: &mut Vec<Value>) {
        if !self.having.is_empty() {
            sql.push_str(" HAVING ");

            let conditions: Vec<String> = self
                .having
                .iter()
                .map(|c| c.to_sql(params))
                .collect();

            sql.push_str(&conditions.join(" AND "));
        }
    }

    fn append_order_by(&self, sql: &mut String) {
        if !self.order_by.is_empty() {
            sql.push_str(" ORDER BY ");

            let orders: Vec<String> = self
                .order_by
                .iter()
                .map(|o| format!("{} {}", o.column, o.direction.as_str()))
                .collect();

            sql.push_str(&orders.join(", "));
        }
    }

    fn append_limit_offset(&self, sql: &mut String) {
        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = self.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }
    }
}

// =============================================================================
// Condition
// =============================================================================

/// A query condition.
#[derive(Debug, Clone)]
pub enum Condition {
    Eq(String, Value),
    Ne(String, Value),
    Gt(String, Value),
    Gte(String, Value),
    Lt(String, Value),
    Lte(String, Value),
    Like(String, String),
    In(String, Vec<Value>),
    IsNull(String),
    IsNotNull(String),
    Raw(String),
}

impl Condition {
    fn to_sql(&self, params: &mut Vec<Value>) -> String {
        match self {
            Self::Eq(col, val) => {
                params.push(val.clone());
                format!("{} = ${}", col, params.len())
            }
            Self::Ne(col, val) => {
                params.push(val.clone());
                format!("{} != ${}", col, params.len())
            }
            Self::Gt(col, val) => {
                params.push(val.clone());
                format!("{} > ${}", col, params.len())
            }
            Self::Gte(col, val) => {
                params.push(val.clone());
                format!("{} >= ${}", col, params.len())
            }
            Self::Lt(col, val) => {
                params.push(val.clone());
                format!("{} < ${}", col, params.len())
            }
            Self::Lte(col, val) => {
                params.push(val.clone());
                format!("{} <= ${}", col, params.len())
            }
            Self::Like(col, pattern) => {
                params.push(Value::String(pattern.clone()));
                format!("{} LIKE ${}", col, params.len())
            }
            Self::In(col, vals) => {
                let placeholders: Vec<String> = vals
                    .iter()
                    .map(|v| {
                        params.push(v.clone());
                        format!("${}", params.len())
                    })
                    .collect();
                format!("{} IN ({})", col, placeholders.join(", "))
            }
            Self::IsNull(col) => format!("{} IS NULL", col),
            Self::IsNotNull(col) => format!("{} IS NOT NULL", col),
            Self::Raw(sql) => sql.clone(),
        }
    }
}

// =============================================================================
// Join
// =============================================================================

#[derive(Debug, Clone)]
struct Join {
    join_type: JoinType,
    table: String,
    on: String,
}

#[derive(Debug, Clone)]
enum JoinType {
    Inner,
    Left,
    Right,
    Full,
}

impl JoinType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Inner => "INNER",
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
            Self::Full => "FULL",
        }
    }
}

// =============================================================================
// Order
// =============================================================================

#[derive(Debug, Clone)]
struct OrderBy {
    column: String,
    direction: OrderDirection,
}

/// Order direction for ORDER BY.
#[derive(Debug, Clone, Copy)]
pub enum OrderDirection {
    Asc,
    Desc,
}

impl OrderDirection {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_select() {
        let query = QueryBuilder::new()
            .select(&["id", "name"])
            .from("users")
            .build();

        assert_eq!(query.sql, "SELECT id, name FROM users");
    }

    #[test]
    fn test_select_with_where() {
        let query = QueryBuilder::new()
            .select(&["*"])
            .from("users")
            .where_eq("id", 1)
            .build();

        assert_eq!(query.sql, "SELECT * FROM users WHERE id = $1");
        assert_eq!(query.params.len(), 1);
    }

    #[test]
    fn test_select_with_order_limit() {
        let query = QueryBuilder::new()
            .select(&["*"])
            .from("users")
            .order_by_desc("created_at")
            .limit(10)
            .offset(20)
            .build();

        assert!(query.sql.contains("ORDER BY created_at DESC"));
        assert!(query.sql.contains("LIMIT 10"));
        assert!(query.sql.contains("OFFSET 20"));
    }

    #[test]
    fn test_insert() {
        let query = QueryBuilder::new()
            .insert_into("users")
            .columns(&["name", "email"])
            .values(vec![Value::String("Alice".to_string()), Value::String("alice@example.com".to_string())])
            .build();

        assert!(query.sql.starts_with("INSERT INTO users"));
        assert!(query.sql.contains("(name, email)"));
        assert!(query.sql.contains("VALUES ($1, $2)"));
    }

    #[test]
    fn test_update() {
        let query = QueryBuilder::new()
            .update("users")
            .set("name", "Bob")
            .set("age", 30)
            .where_eq("id", 1)
            .build();

        assert!(query.sql.starts_with("UPDATE users SET"));
        assert!(query.sql.contains("name = $1"));
        assert!(query.sql.contains("WHERE id = $3"));
    }

    #[test]
    fn test_delete() {
        let query = QueryBuilder::new()
            .delete_from("users")
            .where_eq("id", 1)
            .build();

        assert_eq!(query.sql, "DELETE FROM users WHERE id = $1");
    }

    #[test]
    fn test_join() {
        let query = QueryBuilder::new()
            .select(&["users.name", "orders.total"])
            .from("users")
            .join("orders", "users.id = orders.user_id")
            .build();

        assert!(query.sql.contains("INNER JOIN orders ON users.id = orders.user_id"));
    }

    #[test]
    fn test_where_in() {
        let query = QueryBuilder::new()
            .select(&["*"])
            .from("users")
            .where_in("id", vec![Value::Int(1), Value::Int(2), Value::Int(3)])
            .build();

        assert!(query.sql.contains("id IN ($1, $2, $3)"));
        assert_eq!(query.params.len(), 3);
    }

    #[test]
    fn test_where_null() {
        let query = QueryBuilder::new()
            .select(&["*"])
            .from("users")
            .where_null("deleted_at")
            .build();

        assert!(query.sql.contains("deleted_at IS NULL"));
    }
}
