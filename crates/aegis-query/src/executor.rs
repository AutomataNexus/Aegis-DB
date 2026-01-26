//! Aegis Query Executor
//!
//! Executes query plans using a Volcano-style iterator model.
//! Implements vectorized execution for improved performance.
//!
//! Key Features:
//! - Pull-based iterator execution
//! - Vectorized batch processing
//! - Memory-efficient result streaming
//! - Support for parallel execution
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::planner::{
    AggregateExpr, AggregateFunction, AggregateNode, AlterTableNode, CreateIndexNode,
    CreateTableConstraint, CreateTableNode, DeleteNode, DropIndexNode, DropTableNode, FilterNode,
    InsertNode, InsertPlanSource, JoinNode, JoinStrategy, LimitNode, PlanAlterOperation,
    PlanBinaryOp, PlanExpression, PlanJoinType, PlanLiteral, PlanNode, PlanUnaryOp, ProjectNode,
    ProjectionExpr, QueryPlan, ScanNode, SortKey, SortNode, UpdateNode,
};
use crate::index::{TableIndexManager, IndexType, IndexKey, IndexError};
use aegis_common::{DataType, Row, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

// =============================================================================
// Error Types
// =============================================================================

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Table not found: {0}")]
    TableNotFound(String),

    #[error("Column not found: {0}")]
    ColumnNotFound(String),

    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Execution error: {0}")]
    Internal(String),

    #[error("Index error: {0}")]
    IndexError(String),
}

impl From<IndexError> for ExecutorError {
    fn from(e: IndexError) -> Self {
        ExecutorError::IndexError(e.to_string())
    }
}

pub type ExecutorResult<T> = Result<T, ExecutorError>;

// =============================================================================
// Execution Context
// =============================================================================

/// Context for query execution.
pub struct ExecutionContext {
    tables: HashMap<String, Arc<RwLock<TableData>>>,
    table_schemas: HashMap<String, TableSchema>,
    /// Index metadata (for persistence)
    indexes: HashMap<String, Vec<IndexSchema>>,
    /// Actual index data structures
    table_indexes: HashMap<String, Arc<TableIndexManager>>,
    batch_size: usize,
}

/// In-memory table data for execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TableData {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
}

/// Stored table constraint.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredConstraint {
    pub name: String,
    pub constraint_type: StoredConstraintType,
}

/// Stored constraint type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum StoredConstraintType {
    PrimaryKey { columns: Vec<String> },
    Unique { columns: Vec<String> },
    ForeignKey {
        columns: Vec<String>,
        ref_table: String,
        ref_columns: Vec<String>,
    },
    Check { expression_text: String },
}

/// Table schema information.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnSchema>,
    pub primary_key: Option<Vec<String>>,
    pub constraints: Vec<StoredConstraint>,
}

/// Column schema information.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default: Option<Value>,
}

/// Index schema information.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexSchema {
    pub name: String,
    pub table: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

/// Serializable snapshot of the execution context for persistence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionContextSnapshot {
    pub tables: Vec<TableData>,
    pub schemas: Vec<TableSchema>,
    pub indexes: HashMap<String, Vec<IndexSchema>>,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
            table_schemas: HashMap::new(),
            indexes: HashMap::new(),
            table_indexes: HashMap::new(),
            batch_size: 1024,
        }
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub fn add_table(&mut self, table: TableData) {
        self.tables.insert(table.name.clone(), Arc::new(RwLock::new(table)));
    }

    pub fn get_table(&self, name: &str) -> Option<Arc<RwLock<TableData>>> {
        self.tables.get(name).cloned()
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    // ==========================================================================
    // DDL Operations
    // ==========================================================================

    /// Create a new table.
    pub fn create_table(
        &mut self,
        name: String,
        columns: Vec<ColumnSchema>,
        primary_key: Option<Vec<String>>,
        constraints: Vec<StoredConstraint>,
        if_not_exists: bool,
    ) -> ExecutorResult<()> {
        if self.tables.contains_key(&name) {
            if if_not_exists {
                return Ok(());
            }
            return Err(ExecutorError::InvalidOperation(format!(
                "Table '{}' already exists",
                name
            )));
        }

        let column_names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();

        // Create table schema
        let schema = TableSchema {
            name: name.clone(),
            columns,
            primary_key,
            constraints,
        };
        self.table_schemas.insert(name.clone(), schema);

        // Create empty table data
        let table_data = TableData {
            name: name.clone(),
            columns: column_names,
            rows: Vec::new(),
        };
        self.tables.insert(name, Arc::new(RwLock::new(table_data)));

        Ok(())
    }

    /// Drop a table.
    pub fn drop_table(&mut self, name: &str, if_exists: bool) -> ExecutorResult<()> {
        if !self.tables.contains_key(name) {
            if if_exists {
                return Ok(());
            }
            return Err(ExecutorError::TableNotFound(name.to_string()));
        }

        self.tables.remove(name);
        self.table_schemas.remove(name);
        self.indexes.remove(name);

        Ok(())
    }

    /// Alter a table.
    pub fn alter_table(&mut self, name: &str, op: &PlanAlterOperation) -> ExecutorResult<()> {
        let schema = self.table_schemas.get_mut(name)
            .ok_or_else(|| ExecutorError::TableNotFound(name.to_string()))?;

        match op {
            PlanAlterOperation::AddColumn(col) => {
                // Check if column already exists
                if schema.columns.iter().any(|c| c.name == col.name) {
                    return Err(ExecutorError::InvalidOperation(format!(
                        "Column '{}' already exists in table '{}'",
                        col.name, name
                    )));
                }

                // Evaluate default expression if provided
                let default = col.default.as_ref()
                    .map(|expr| evaluate_default_expression(expr))
                    .transpose()?;

                schema.columns.push(ColumnSchema {
                    name: col.name.clone(),
                    data_type: col.data_type.clone(),
                    nullable: col.nullable,
                    default: default.clone(),
                });

                // Add default (or null) values to existing rows
                let fill_value = default.unwrap_or(Value::Null);
                if let Some(table_data) = self.tables.get(name) {
                    let mut table = table_data.write().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;
                    for row in table.rows.iter_mut() {
                        row.values.push(fill_value.clone());
                    }
                }
            }
            PlanAlterOperation::DropColumn { name: col_name, if_exists } => {
                let col_idx = schema.columns.iter().position(|c| c.name == *col_name);
                match col_idx {
                    Some(idx) => {
                        schema.columns.remove(idx);
                        // Remove values from existing rows
                        if let Some(table_data) = self.tables.get(name) {
                            let mut table = table_data.write().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;
                            for row in table.rows.iter_mut() {
                                if idx < row.values.len() {
                                    row.values.remove(idx);
                                }
                            }
                        }
                    }
                    None if *if_exists => {}
                    None => {
                        return Err(ExecutorError::ColumnNotFound(col_name.clone()));
                    }
                }
            }
            PlanAlterOperation::RenameColumn { old_name, new_name } => {
                let col = schema.columns.iter_mut().find(|c| c.name == *old_name)
                    .ok_or_else(|| ExecutorError::ColumnNotFound(old_name.clone()))?;
                col.name = new_name.clone();
            }
            PlanAlterOperation::AlterColumn { name: col_name, data_type, set_not_null, set_default } => {
                let col = schema.columns.iter_mut().find(|c| c.name == *col_name)
                    .ok_or_else(|| ExecutorError::ColumnNotFound(col_name.clone()))?;

                if let Some(dt) = data_type {
                    col.data_type = dt.clone();
                }
                if let Some(not_null) = set_not_null {
                    col.nullable = !not_null;
                }
                // Handle set_default: None = no change, Some(None) = drop default, Some(Some(expr)) = set new default
                if let Some(new_default) = set_default {
                    col.default = new_default.as_ref()
                        .map(|expr| evaluate_default_expression(expr))
                        .transpose()?;
                }
            }
            PlanAlterOperation::RenameTable { new_name } => {
                // Move data to new table name
                if let Some(rows) = self.tables.remove(name) {
                    self.tables.insert(new_name.clone(), rows);
                }
                if let Some(mut schema) = self.table_schemas.remove(name) {
                    schema.name = new_name.clone();
                    self.table_schemas.insert(new_name.clone(), schema);
                }
                if let Some(indexes) = self.indexes.remove(name) {
                    self.indexes.insert(new_name.clone(), indexes);
                }
                // Return early since name is now invalid
                return Ok(());
            }
            PlanAlterOperation::AddConstraint(constraint) => {
                // Pre-validate foreign key reference before getting mutable borrow
                if let CreateTableConstraint::ForeignKey { ref_table, .. } = constraint {
                    if !self.table_schemas.contains_key(ref_table) {
                        return Err(ExecutorError::TableNotFound(ref_table.clone()));
                    }
                }

                // Now safe to get mutable reference for the rest of the operation
                let schema = self.table_schemas.get_mut(name)
                    .ok_or_else(|| ExecutorError::TableNotFound(name.to_string()))?;

                let constraint_count = schema.constraints.len() + 1;
                let (name_suffix, constraint_type) = match constraint {
                    CreateTableConstraint::PrimaryKey { columns } => {
                        // Check if primary key already exists
                        if schema.primary_key.is_some() {
                            return Err(ExecutorError::InvalidOperation(
                                format!("Table '{}' already has a primary key", name)
                            ));
                        }
                        schema.primary_key = Some(columns.clone());
                        ("pk", StoredConstraintType::PrimaryKey { columns: columns.clone() })
                    }
                    CreateTableConstraint::Unique { columns } => {
                        // Validate columns exist
                        for col in columns {
                            if !schema.columns.iter().any(|c| c.name == *col) {
                                return Err(ExecutorError::ColumnNotFound(col.clone()));
                            }
                        }
                        ("uq", StoredConstraintType::Unique { columns: columns.clone() })
                    }
                    CreateTableConstraint::ForeignKey { columns, ref_table, ref_columns } => {
                        // Validate columns exist
                        for col in columns {
                            if !schema.columns.iter().any(|c| c.name == *col) {
                                return Err(ExecutorError::ColumnNotFound(col.clone()));
                            }
                        }
                        // ref_table validated above
                        ("fk", StoredConstraintType::ForeignKey {
                            columns: columns.clone(),
                            ref_table: ref_table.clone(),
                            ref_columns: ref_columns.clone(),
                        })
                    }
                    CreateTableConstraint::Check { expression } => {
                        ("ck", StoredConstraintType::Check {
                            expression_text: format!("{:?}", expression),
                        })
                    }
                };
                schema.constraints.push(StoredConstraint {
                    name: format!("{}_{}{}", name, name_suffix, constraint_count),
                    constraint_type,
                });
                return Ok(());
            }
            PlanAlterOperation::DropConstraint { name: constraint_name } => {
                let pos = schema.constraints.iter().position(|c| c.name == *constraint_name);
                match pos {
                    Some(idx) => {
                        let removed = schema.constraints.remove(idx);
                        // If dropping primary key constraint, also clear primary_key field
                        if matches!(removed.constraint_type, StoredConstraintType::PrimaryKey { .. }) {
                            schema.primary_key = None;
                        }
                    }
                    None => {
                        return Err(ExecutorError::InvalidOperation(
                            format!("Constraint '{}' not found on table '{}'", constraint_name, name)
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Create an index.
    pub fn create_index(
        &mut self,
        name: String,
        table: String,
        columns: Vec<String>,
        unique: bool,
        if_not_exists: bool,
    ) -> ExecutorResult<()> {
        let table_data = self.tables.get(&table)
            .ok_or_else(|| ExecutorError::TableNotFound(table.clone()))?
            .clone();

        let indexes = self.indexes.entry(table.clone()).or_default();

        // Check if index already exists
        if indexes.iter().any(|idx| idx.name == name) {
            if if_not_exists {
                return Ok(());
            }
            return Err(ExecutorError::InvalidOperation(format!(
                "Index '{}' already exists",
                name
            )));
        }

        // Store metadata
        indexes.push(IndexSchema {
            name: name.clone(),
            table: table.clone(),
            columns: columns.clone(),
            unique,
        });

        // Create actual index structure
        let index_manager = self.table_indexes.entry(table.clone())
            .or_insert_with(|| Arc::new(TableIndexManager::new(table.clone())));

        // Use B-tree by default (supports range queries)
        Arc::get_mut(index_manager)
            .ok_or_else(|| ExecutorError::Internal("Cannot modify shared index manager".to_string()))?
            .create_index(name.clone(), columns.clone(), unique, IndexType::BTree)?;

        // Populate the index with existing data
        let table_guard = table_data.read()
            .map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;

        let index_manager = self.table_indexes.get(&table).unwrap();
        for (row_id, row) in table_guard.rows.iter().enumerate() {
            let column_values: HashMap<String, Value> = table_guard.columns.iter()
                .zip(row.values.iter())
                .map(|(col, val)| (col.clone(), val.clone()))
                .collect();

            // Build index key from specified columns
            let key_values: Vec<crate::index::IndexValue> = columns.iter()
                .map(|col| {
                    column_values.get(col)
                        .map(IndexKey::from_value)
                        .unwrap_or(crate::index::IndexValue::Null)
                })
                .collect();
            let key = IndexKey::new(key_values);

            // Insert into the index using public API
            index_manager.insert_into_index(&name, key, row_id)?;
        }

        Ok(())
    }

    /// Drop an index.
    pub fn drop_index(&mut self, name: &str, if_exists: bool) -> ExecutorResult<()> {
        let mut found = false;
        let mut table_name = None;

        // Find and remove from metadata
        for (tbl, indexes) in self.indexes.iter_mut() {
            if let Some(pos) = indexes.iter().position(|idx| idx.name == name) {
                indexes.remove(pos);
                table_name = Some(tbl.clone());
                found = true;
                break;
            }
        }

        // Remove from actual index structure
        if let Some(tbl) = table_name {
            if let Some(manager) = self.table_indexes.get_mut(&tbl) {
                if let Some(m) = Arc::get_mut(manager) {
                    let _ = m.drop_index(name);
                }
            }
        }

        if !found && !if_exists {
            return Err(ExecutorError::InvalidOperation(format!(
                "Index '{}' not found",
                name
            )));
        }

        Ok(())
    }

    /// Get the index manager for a table.
    pub fn get_index_manager(&self, table: &str) -> Option<Arc<TableIndexManager>> {
        self.table_indexes.get(table).cloned()
    }

    /// Get index metadata for a table.
    pub fn get_indexes(&self, table: &str) -> Option<&Vec<IndexSchema>> {
        self.indexes.get(table)
    }

    /// Get table schema.
    pub fn get_table_schema(&self, name: &str) -> Option<&TableSchema> {
        self.table_schemas.get(name)
    }

    /// List all table names.
    pub fn list_tables(&self) -> Vec<String> {
        self.tables.keys().cloned().collect()
    }

    // ==========================================================================
    // Persistence Operations
    // ==========================================================================

    /// Create a serializable snapshot of the execution context.
    pub fn to_snapshot(&self) -> ExecutionContextSnapshot {
        let tables: Vec<TableData> = self.tables.values()
            .filter_map(|t| t.read().ok().map(|t| t.clone()))
            .collect();

        ExecutionContextSnapshot {
            tables,
            schemas: self.table_schemas.values().cloned().collect(),
            indexes: self.indexes.clone(),
        }
    }

    /// Restore execution context from a snapshot.
    pub fn from_snapshot(snapshot: ExecutionContextSnapshot) -> Self {
        let mut ctx = Self::new();

        // Restore schemas first
        for schema in snapshot.schemas {
            ctx.table_schemas.insert(schema.name.clone(), schema);
        }

        // Restore tables
        for table in snapshot.tables {
            ctx.tables.insert(table.name.clone(), Arc::new(RwLock::new(table)));
        }

        // Store index metadata
        ctx.indexes = snapshot.indexes.clone();

        // Rebuild actual index structures from metadata
        for (table_name, index_schemas) in snapshot.indexes {
            for index_schema in index_schemas {
                // Rebuild each index
                let _ = ctx.rebuild_index(
                    &index_schema.name,
                    &table_name,
                    &index_schema.columns,
                    index_schema.unique,
                );
            }
        }

        ctx
    }

    /// Rebuild an index from existing table data.
    fn rebuild_index(
        &mut self,
        name: &str,
        table: &str,
        columns: &[String],
        unique: bool,
    ) -> ExecutorResult<()> {
        let table_data = self.tables.get(table)
            .ok_or_else(|| ExecutorError::TableNotFound(table.to_string()))?
            .clone();

        // Create index manager if needed
        let index_manager = self.table_indexes.entry(table.to_string())
            .or_insert_with(|| Arc::new(TableIndexManager::new(table.to_string())));

        // Create the index structure
        if let Some(m) = Arc::get_mut(index_manager) {
            m.create_index(name.to_string(), columns.to_vec(), unique, IndexType::BTree)?;
        } else {
            return Err(ExecutorError::Internal("Cannot modify shared index manager".to_string()));
        }

        // Populate with existing data
        let table_guard = table_data.read()
            .map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;

        let index_manager = self.table_indexes.get(table).unwrap();
        for (row_id, row) in table_guard.rows.iter().enumerate() {
            let column_values: HashMap<String, Value> = table_guard.columns.iter()
                .zip(row.values.iter())
                .map(|(col, val)| (col.clone(), val.clone()))
                .collect();

            let key_values: Vec<crate::index::IndexValue> = columns.iter()
                .map(|col| {
                    column_values.get(col)
                        .map(IndexKey::from_value)
                        .unwrap_or(crate::index::IndexValue::Null)
                })
                .collect();
            let key = IndexKey::new(key_values);

            index_manager.insert_into_index(name, key, row_id)?;
        }

        Ok(())
    }

    /// Save the execution context to a file.
    pub fn save_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        let snapshot = self.to_snapshot();
        let json = serde_json::to_string_pretty(&snapshot)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    /// Load the execution context from a file.
    pub fn load_from_file(path: &std::path::Path) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let snapshot: ExecutionContextSnapshot = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(Self::from_snapshot(snapshot))
    }

    /// Merge data from a file into the current context (for loading on startup).
    pub fn merge_from_file(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        let json = std::fs::read_to_string(path)?;
        let snapshot: ExecutionContextSnapshot = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Merge schemas
        for schema in snapshot.schemas {
            self.table_schemas.insert(schema.name.clone(), schema);
        }

        // Merge tables
        for table in snapshot.tables {
            self.tables.insert(table.name.clone(), Arc::new(RwLock::new(table)));
        }

        // Merge indexes
        for (table, idxs) in snapshot.indexes {
            self.indexes.insert(table, idxs);
        }

        Ok(())
    }

    // ==========================================================================
    // DML Operations
    // ==========================================================================

    /// Insert rows into a table.
    pub fn insert_rows(
        &self,
        table_name: &str,
        columns: &[String],
        rows: Vec<Vec<Value>>,
    ) -> ExecutorResult<u64> {
        let table = self.get_table(table_name)
            .ok_or_else(|| ExecutorError::TableNotFound(table_name.to_string()))?;

        let mut table_data = table.write()
            .map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;

        // Get schema to access column defaults
        let schema = self.table_schemas.get(table_name)
            .ok_or_else(|| ExecutorError::TableNotFound(table_name.to_string()))?;

        // If no columns specified, use all columns in order
        let target_columns: Vec<String> = if columns.is_empty() {
            table_data.columns.clone()
        } else {
            columns.to_vec()
        };

        // Build column index mapping
        let mut column_indices: Vec<usize> = Vec::new();
        for col in &target_columns {
            let idx = table_data.columns.iter().position(|c| c == col)
                .ok_or_else(|| ExecutorError::ColumnNotFound(col.clone()))?;
            column_indices.push(idx);
        }

        let mut inserted = 0u64;

        // Build default values for each column
        let defaults: Vec<Value> = schema.columns.iter()
            .map(|col| col.default.clone().unwrap_or(Value::Null))
            .collect();

        for row_values in rows {
            // Create a new row initialized with defaults (or Null if no default)
            let mut new_row: Vec<Value> = defaults.clone();

            // Fill in the provided values
            for (i, &col_idx) in column_indices.iter().enumerate() {
                if let Some(value) = row_values.get(i) {
                    new_row[col_idx] = value.clone();
                }
            }

            table_data.rows.push(Row { values: new_row });
            inserted += 1;
        }

        Ok(inserted)
    }

    /// Update rows in a table.
    pub fn update_rows(
        &self,
        table_name: &str,
        assignments: &[(String, Value)],
        predicate: Option<&dyn Fn(&Row, &[String]) -> bool>,
    ) -> ExecutorResult<u64> {
        let table = self.get_table(table_name)
            .ok_or_else(|| ExecutorError::TableNotFound(table_name.to_string()))?;

        let mut table_data = table.write()
            .map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;
        let columns = table_data.columns.clone();

        // Build assignment index mapping
        let mut assignment_indices: Vec<(usize, Value)> = Vec::new();
        for (col, val) in assignments {
            let idx = columns.iter().position(|c| c == col)
                .ok_or_else(|| ExecutorError::ColumnNotFound(col.clone()))?;
            assignment_indices.push((idx, val.clone()));
        }

        let mut updated = 0u64;

        for row in &mut table_data.rows {
            let should_update = predicate.map(|p| p(row, &columns)).unwrap_or(true);
            if should_update {
                for (col_idx, value) in &assignment_indices {
                    row.values[*col_idx] = value.clone();
                }
                updated += 1;
            }
        }

        Ok(updated)
    }

    /// Delete rows from a table.
    pub fn delete_rows(
        &self,
        table_name: &str,
        predicate: Option<&dyn Fn(&Row, &[String]) -> bool>,
    ) -> ExecutorResult<u64> {
        let table = self.get_table(table_name)
            .ok_or_else(|| ExecutorError::TableNotFound(table_name.to_string()))?;

        let mut table_data = table.write()
            .map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;
        let columns = table_data.columns.clone();

        let original_len = table_data.rows.len();

        if let Some(pred) = predicate {
            table_data.rows.retain(|row| !pred(row, &columns));
        } else {
            table_data.rows.clear();
        }

        Ok((original_len - table_data.rows.len()) as u64)
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Result Batch
// =============================================================================

/// A batch of result rows.
#[derive(Debug, Clone)]
pub struct ResultBatch {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
}

impl ResultBatch {
    pub fn new(columns: Vec<String>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
        }
    }

    pub fn with_rows(columns: Vec<String>, rows: Vec<Row>) -> Self {
        Self { columns, rows }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }
}

// =============================================================================
// Query Result
// =============================================================================

/// Complete query result.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    pub rows_affected: u64,
}

impl QueryResult {
    pub fn new(columns: Vec<String>, rows: Vec<Row>) -> Self {
        let rows_affected = rows.len() as u64;
        Self {
            columns,
            rows,
            rows_affected,
        }
    }

    pub fn empty() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: 0,
        }
    }
}

// =============================================================================
// Executor
// =============================================================================

/// Query executor.
pub struct Executor {
    context: Arc<RwLock<ExecutionContext>>,
}

impl Executor {
    pub fn new(context: ExecutionContext) -> Self {
        Self { context: Arc::new(RwLock::new(context)) }
    }

    /// Create executor with a shared context (for persistent DDL across queries).
    pub fn with_shared_context(context: Arc<RwLock<ExecutionContext>>) -> Self {
        Self { context }
    }

    /// Execute a query plan.
    pub fn execute(&self, plan: &QueryPlan) -> ExecutorResult<QueryResult> {
        self.execute_internal(&plan.root)
    }

    /// Execute a query plan with bound parameters.
    /// Parameters are indexed starting from 1 (e.g., $1, $2, etc.)
    pub fn execute_with_params(&self, plan: &QueryPlan, params: &[Value]) -> ExecutorResult<QueryResult> {
        // Substitute placeholders with actual parameter values
        let bound_plan = substitute_parameters(&plan.root, params)?;
        self.execute_internal(&bound_plan)
    }

    fn execute_internal(&self, root: &PlanNode) -> ExecutorResult<QueryResult> {
        match root {
            // DDL operations
            PlanNode::CreateTable(node) => self.execute_create_table(node),
            PlanNode::DropTable(node) => self.execute_drop_table(node),
            PlanNode::AlterTable(node) => self.execute_alter_table(node),
            PlanNode::CreateIndex(node) => self.execute_create_index(node),
            PlanNode::DropIndex(node) => self.execute_drop_index(node),

            // DML operations
            PlanNode::Insert(node) => self.execute_insert(node),
            PlanNode::Update(node) => self.execute_update(node),
            PlanNode::Delete(node) => self.execute_delete(node),

            // Query operations (SELECT)
            _ => self.execute_query(root),
        }
    }

    /// Execute a SELECT query.
    fn execute_query(&self, root: &PlanNode) -> ExecutorResult<QueryResult> {
        let context = self.context.read().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;
        let mut operator = self.create_operator(root, &context)?;
        let mut all_rows = Vec::new();
        let mut columns = Vec::new();

        while let Some(batch) = operator.next_batch()? {
            if columns.is_empty() {
                columns = batch.columns.clone();
            }
            all_rows.extend(batch.rows);
        }

        Ok(QueryResult::new(columns, all_rows))
    }

    /// Execute CREATE TABLE.
    fn execute_create_table(&self, node: &CreateTableNode) -> ExecutorResult<QueryResult> {
        let columns: Vec<ColumnSchema> = node.columns.iter().map(|col| {
            let default = col.default.as_ref()
                .map(|expr| evaluate_default_expression(expr))
                .transpose()?;
            Ok(ColumnSchema {
                name: col.name.clone(),
                data_type: col.data_type.clone(),
                nullable: col.nullable,
                default,
            })
        }).collect::<ExecutorResult<Vec<_>>>()?;

        // Extract primary key from constraints
        let primary_key = node.constraints.iter()
            .find_map(|c| {
                if let CreateTableConstraint::PrimaryKey { columns } = c {
                    Some(columns.clone())
                } else {
                    None
                }
            })
            .or_else(|| {
                // Check column-level primary key
                let pk_cols: Vec<String> = node.columns.iter()
                    .filter(|c| c.primary_key)
                    .map(|c| c.name.clone())
                    .collect();
                if pk_cols.is_empty() { None } else { Some(pk_cols) }
            });

        // Convert plan constraints to stored constraints with auto-generated names
        let mut stored_constraints = Vec::new();
        let mut constraint_counter = 0;
        for c in &node.constraints {
            constraint_counter += 1;
            let (name_suffix, constraint_type) = match c {
                CreateTableConstraint::PrimaryKey { columns } => {
                    ("pk", StoredConstraintType::PrimaryKey { columns: columns.clone() })
                }
                CreateTableConstraint::Unique { columns } => {
                    ("uq", StoredConstraintType::Unique { columns: columns.clone() })
                }
                CreateTableConstraint::ForeignKey { columns, ref_table, ref_columns } => {
                    ("fk", StoredConstraintType::ForeignKey {
                        columns: columns.clone(),
                        ref_table: ref_table.clone(),
                        ref_columns: ref_columns.clone(),
                    })
                }
                CreateTableConstraint::Check { expression } => {
                    ("ck", StoredConstraintType::Check {
                        expression_text: format!("{:?}", expression),
                    })
                }
            };
            stored_constraints.push(StoredConstraint {
                name: format!("{}_{}{}", node.table_name, name_suffix, constraint_counter),
                constraint_type,
            });
        }

        // Add column-level unique constraints
        for col in &node.columns {
            if col.unique {
                constraint_counter += 1;
                stored_constraints.push(StoredConstraint {
                    name: format!("{}_{}_uq{}", node.table_name, col.name, constraint_counter),
                    constraint_type: StoredConstraintType::Unique { columns: vec![col.name.clone()] },
                });
            }
        }

        self.context.write().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?.create_table(
            node.table_name.clone(),
            columns,
            primary_key,
            stored_constraints,
            node.if_not_exists,
        )?;

        Ok(QueryResult {
            columns: vec!["result".to_string()],
            rows: vec![Row { values: vec![Value::String(format!("Table '{}' created", node.table_name))] }],
            rows_affected: 0,
        })
    }

    /// Execute DROP TABLE.
    fn execute_drop_table(&self, node: &DropTableNode) -> ExecutorResult<QueryResult> {
        self.context.write().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?.drop_table(&node.table_name, node.if_exists)?;

        Ok(QueryResult {
            columns: vec!["result".to_string()],
            rows: vec![Row { values: vec![Value::String(format!("Table '{}' dropped", node.table_name))] }],
            rows_affected: 0,
        })
    }

    /// Execute ALTER TABLE.
    fn execute_alter_table(&self, node: &AlterTableNode) -> ExecutorResult<QueryResult> {
        let mut ctx = self.context.write().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;

        for op in &node.operations {
            ctx.alter_table(&node.table_name, op)?;
        }

        let op_count = node.operations.len();
        let msg = if op_count == 1 {
            format!("Table '{}' altered", node.table_name)
        } else {
            format!("Table '{}' altered ({} operations)", node.table_name, op_count)
        };

        Ok(QueryResult {
            columns: vec!["result".to_string()],
            rows: vec![Row { values: vec![Value::String(msg)] }],
            rows_affected: 0,
        })
    }

    /// Execute CREATE INDEX.
    fn execute_create_index(&self, node: &CreateIndexNode) -> ExecutorResult<QueryResult> {
        self.context.write().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?.create_index(
            node.index_name.clone(),
            node.table_name.clone(),
            node.columns.clone(),
            node.unique,
            node.if_not_exists,
        )?;

        Ok(QueryResult {
            columns: vec!["result".to_string()],
            rows: vec![Row { values: vec![Value::String(format!("Index '{}' created", node.index_name))] }],
            rows_affected: 0,
        })
    }

    /// Execute DROP INDEX.
    fn execute_drop_index(&self, node: &DropIndexNode) -> ExecutorResult<QueryResult> {
        self.context.write().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?.drop_index(&node.index_name, node.if_exists)?;

        Ok(QueryResult {
            columns: vec!["result".to_string()],
            rows: vec![Row { values: vec![Value::String(format!("Index '{}' dropped", node.index_name))] }],
            rows_affected: 0,
        })
    }

    /// Execute INSERT.
    fn execute_insert(&self, node: &InsertNode) -> ExecutorResult<QueryResult> {
        let rows: Vec<Vec<Value>> = match &node.source {
            InsertPlanSource::Values(values) => {
                // Evaluate all value expressions
                let empty_row = Row { values: vec![] };
                let empty_columns: Vec<String> = vec![];

                values.iter()
                    .map(|row_exprs| {
                        row_exprs.iter()
                            .map(|expr| evaluate_expression(expr, &empty_row, &empty_columns))
                            .collect::<ExecutorResult<Vec<_>>>()
                    })
                    .collect::<ExecutorResult<Vec<_>>>()?
            }
            InsertPlanSource::Query(subquery) => {
                // Execute the subquery and use its results as the insert data
                let context = self.context.read()
                    .map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;
                let mut operator = self.create_operator(subquery, &context)?;
                let mut all_rows = Vec::new();

                while let Some(batch) = operator.next_batch()? {
                    for row in batch.rows {
                        all_rows.push(row.values);
                    }
                }

                all_rows
            }
        };

        let inserted = self.context.read()
            .map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?
            .insert_rows(&node.table_name, &node.columns, rows)?;

        Ok(QueryResult {
            columns: vec!["rows_affected".to_string()],
            rows: vec![Row { values: vec![Value::Integer(inserted as i64)] }],
            rows_affected: inserted,
        })
    }

    /// Execute UPDATE.
    fn execute_update(&self, node: &UpdateNode) -> ExecutorResult<QueryResult> {
        let context = self.context.read().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;

        // Get table columns for expression evaluation
        let table = context.get_table(&node.table_name)
            .ok_or_else(|| ExecutorError::TableNotFound(node.table_name.clone()))?;
        let table_data = table.read()
            .map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;
        let _columns = table_data.columns.clone();
        drop(table_data);

        // Evaluate assignment values
        let empty_row = Row { values: vec![] };
        let assignments: Vec<(String, Value)> = node.assignments.iter()
            .map(|(col, expr)| {
                let value = evaluate_expression(expr, &empty_row, &[])?;
                Ok((col.clone(), value))
            })
            .collect::<ExecutorResult<Vec<_>>>()?;

        // Create predicate closure if where clause exists
        let where_clause = node.where_clause.clone();
        let predicate: Option<Box<dyn Fn(&Row, &[String]) -> bool>> = where_clause.map(|wc| {
            Box::new(move |row: &Row, cols: &[String]| {
                evaluate_expression(&wc, row, cols)
                    .map(|v| matches!(v, Value::Boolean(true)))
                    .unwrap_or(false)
            }) as Box<dyn Fn(&Row, &[String]) -> bool>
        });

        let updated = context.update_rows(
            &node.table_name,
            &assignments,
            predicate.as_ref().map(|p| p.as_ref()),
        )?;

        Ok(QueryResult {
            columns: vec!["rows_affected".to_string()],
            rows: vec![Row { values: vec![Value::Integer(updated as i64)] }],
            rows_affected: updated,
        })
    }

    /// Execute DELETE.
    fn execute_delete(&self, node: &DeleteNode) -> ExecutorResult<QueryResult> {
        let context = self.context.read().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;

        // Create predicate closure if where clause exists
        let where_clause = node.where_clause.clone();
        let predicate: Option<Box<dyn Fn(&Row, &[String]) -> bool>> = where_clause.map(|wc| {
            Box::new(move |row: &Row, cols: &[String]| {
                evaluate_expression(&wc, row, cols)
                    .map(|v| matches!(v, Value::Boolean(true)))
                    .unwrap_or(false)
            }) as Box<dyn Fn(&Row, &[String]) -> bool>
        });

        let deleted = context.delete_rows(
            &node.table_name,
            predicate.as_ref().map(|p| p.as_ref()),
        )?;

        Ok(QueryResult {
            columns: vec!["rows_affected".to_string()],
            rows: vec![Row { values: vec![Value::Integer(deleted as i64)] }],
            rows_affected: deleted,
        })
    }

    /// Create an operator tree from a plan node.
    fn create_operator<'a>(&'a self, node: &PlanNode, context: &'a ExecutionContext) -> ExecutorResult<Box<dyn Operator + 'a>> {
        match node {
            PlanNode::Scan(scan) => Ok(Box::new(ScanOperator::new(scan.clone(), context)?)),

            PlanNode::Filter(filter) => {
                let input = self.create_operator(&filter.input, context)?;
                Ok(Box::new(FilterOperator::new(
                    input,
                    filter.predicate.clone(),
                )))
            }

            PlanNode::Project(project) => {
                let input = self.create_operator(&project.input, context)?;
                Ok(Box::new(ProjectOperator::new(
                    input,
                    project.expressions.clone(),
                )))
            }

            PlanNode::Join(join) => {
                let left = self.create_operator(&join.left, context)?;
                let right = self.create_operator(&join.right, context)?;
                Ok(Box::new(JoinOperator::new(
                    left,
                    right,
                    join.join_type,
                    join.condition.clone(),
                    join.strategy,
                )?))
            }

            PlanNode::Aggregate(agg) => {
                let input = self.create_operator(&agg.input, context)?;
                Ok(Box::new(AggregateOperator::new(
                    input,
                    agg.group_by.clone(),
                    agg.aggregates.clone(),
                )))
            }

            PlanNode::Sort(sort) => {
                let input = self.create_operator(&sort.input, context)?;
                Ok(Box::new(SortOperator::new(input, sort.order_by.clone())))
            }

            PlanNode::Limit(limit) => {
                let input = self.create_operator(&limit.input, context)?;
                Ok(Box::new(LimitOperator::new(input, limit.limit, limit.offset)))
            }

            PlanNode::Empty => Ok(Box::new(EmptyOperator::new())),

            // DDL and DML nodes are handled in execute(), not here
            PlanNode::CreateTable(_) | PlanNode::DropTable(_) | PlanNode::AlterTable(_) |
            PlanNode::CreateIndex(_) | PlanNode::DropIndex(_) |
            PlanNode::Insert(_) | PlanNode::Update(_) | PlanNode::Delete(_) => {
                Err(ExecutorError::Internal("DDL/DML nodes should not be in operator tree".to_string()))
            }
        }
    }
}

// =============================================================================
// Operator Trait
// =============================================================================

/// Operator in the execution pipeline.
pub trait Operator {
    /// Get the next batch of results.
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>>;

    /// Get output column names.
    fn columns(&self) -> &[String];
}

// =============================================================================
// Scan Operator
// =============================================================================

struct ScanOperator {
    table: Arc<RwLock<TableData>>,
    columns: Vec<String>,
    position: usize,
    batch_size: usize,
    // Cache the rows to avoid repeated locking
    cached_rows: Option<Vec<Row>>,
}

impl ScanOperator {
    fn new(scan: ScanNode, context: &ExecutionContext) -> ExecutorResult<Self> {
        let table = context
            .get_table(&scan.table_name)
            .ok_or_else(|| ExecutorError::TableNotFound(scan.table_name.clone()))?;

        // Read columns under lock, then release lock before moving table into struct
        let columns = {
            let table_data = table.read().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;
            if scan.columns.is_empty() {
                table_data.columns.clone()
            } else {
                scan.columns.clone()
            }
        }; // table_data guard dropped here

        Ok(Self {
            table,
            columns,
            position: 0,
            batch_size: context.batch_size(),
            cached_rows: None,
        })
    }
}

impl Operator for ScanOperator {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        // Cache rows on first access
        if self.cached_rows.is_none() {
            let table_data = self.table.read().map_err(|_| ExecutorError::Internal("Lock poisoned".to_string()))?;
            self.cached_rows = Some(table_data.rows.clone());
        }

        let rows = self.cached_rows.as_ref().unwrap();

        if self.position >= rows.len() {
            return Ok(None);
        }

        let end = (self.position + self.batch_size).min(rows.len());
        let batch_rows: Vec<Row> = rows[self.position..end].to_vec();
        self.position = end;

        Ok(Some(ResultBatch::with_rows(self.columns.clone(), batch_rows)))
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Filter Operator
// =============================================================================

struct FilterOperator<'a> {
    input: Box<dyn Operator + 'a>,
    predicate: PlanExpression,
    columns: Vec<String>,
}

impl<'a> FilterOperator<'a> {
    fn new(input: Box<dyn Operator + 'a>, predicate: PlanExpression) -> Self {
        let columns = input.columns().to_vec();
        Self {
            input,
            predicate,
            columns,
        }
    }

    fn evaluate_predicate(&self, row: &Row, columns: &[String]) -> ExecutorResult<bool> {
        let value = evaluate_expression(&self.predicate, row, columns)?;
        match value {
            Value::Boolean(b) => Ok(b),
            Value::Null => Ok(false),
            _ => Err(ExecutorError::TypeMismatch {
                expected: "boolean".to_string(),
                actual: format!("{:?}", value),
            }),
        }
    }
}

impl<'a> Operator for FilterOperator<'a> {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        while let Some(batch) = self.input.next_batch()? {
            let filtered: Vec<Row> = batch
                .rows
                .into_iter()
                .filter(|row| self.evaluate_predicate(row, &batch.columns).unwrap_or(false))
                .collect();

            if !filtered.is_empty() {
                return Ok(Some(ResultBatch::with_rows(self.columns.clone(), filtered)));
            }
        }
        Ok(None)
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Project Operator
// =============================================================================

struct ProjectOperator<'a> {
    input: Box<dyn Operator + 'a>,
    expressions: Vec<crate::planner::ProjectionExpr>,
    columns: Vec<String>,
    input_columns: Vec<String>,
}

impl<'a> ProjectOperator<'a> {
    fn new(input: Box<dyn Operator + 'a>, expressions: Vec<crate::planner::ProjectionExpr>) -> Self {
        let input_columns = input.columns().to_vec();

        // Expand wildcards in expressions and build column names
        let mut expanded_expressions = Vec::new();
        let mut columns = Vec::new();

        for (i, proj_expr) in expressions.iter().enumerate() {
            // Check if this is a wildcard expression (SELECT *)
            if let PlanExpression::Column { name, table, .. } = &proj_expr.expr {
                if name == "*" {
                    // Expand to all input columns (optionally filtered by table)
                    for input_col in &input_columns {
                        // For table-qualified wildcards (table.*), filter by table prefix
                        if let Some(tbl) = table {
                            // Only include columns that match the table (simplified: check prefix)
                            if !input_col.starts_with(&format!("{}.", tbl)) && input_col != input_col {
                                // For now, include all since we don't have table prefixes in column names
                            }
                        }
                        expanded_expressions.push(crate::planner::ProjectionExpr {
                            expr: PlanExpression::Column {
                                table: None,
                                name: input_col.clone(),
                                data_type: DataType::Any,
                            },
                            alias: None,
                        });
                        columns.push(input_col.clone());
                    }
                    continue;
                }
            }

            // Regular expression
            expanded_expressions.push(proj_expr.clone());
            let col_name = proj_expr.alias.clone().unwrap_or_else(|| {
                extract_column_name(&proj_expr.expr)
                    .unwrap_or_else(|| format!("column_{}", i))
            });
            columns.push(col_name);
        }

        Self {
            input,
            expressions: expanded_expressions,
            columns,
            input_columns,
        }
    }
}

/// Extract a meaningful column name from a PlanExpression.
fn extract_column_name(expr: &PlanExpression) -> Option<String> {
    match expr {
        PlanExpression::Column { name, .. } => Some(name.clone()),
        PlanExpression::Function { name, .. } => Some(name.clone()),
        PlanExpression::Cast { expr, .. } => extract_column_name(expr),
        PlanExpression::UnaryOp { expr, .. } => extract_column_name(expr),
        _ => None,
    }
}

impl<'a> Operator for ProjectOperator<'a> {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        if let Some(batch) = self.input.next_batch()? {
            let mut result_rows = Vec::with_capacity(batch.rows.len());

            for row in &batch.rows {
                let mut projected_values = Vec::with_capacity(self.expressions.len());

                for expr in &self.expressions {
                    let value = evaluate_expression(&expr.expr, row, &self.input_columns)?;
                    projected_values.push(value);
                }

                result_rows.push(Row {
                    values: projected_values,
                });
            }

            Ok(Some(ResultBatch::with_rows(
                self.columns.clone(),
                result_rows,
            )))
        } else {
            Ok(None)
        }
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Join Operator
// =============================================================================

#[allow(dead_code)]
struct JoinOperator<'a> {
    left: Box<dyn Operator + 'a>,
    right: Box<dyn Operator + 'a>,
    join_type: PlanJoinType,
    condition: Option<PlanExpression>,
    _strategy: JoinStrategy,
    columns: Vec<String>,
    left_columns: Vec<String>,
    right_columns: Vec<String>,
    right_data: Option<Vec<Row>>,
    left_batch: Option<ResultBatch>,
    left_row_idx: usize,
    right_row_idx: usize,
}

impl<'a> JoinOperator<'a> {
    fn new(
        left: Box<dyn Operator + 'a>,
        right: Box<dyn Operator + 'a>,
        join_type: PlanJoinType,
        condition: Option<PlanExpression>,
        strategy: JoinStrategy,
    ) -> ExecutorResult<Self> {
        let left_columns = left.columns().to_vec();
        let right_columns = right.columns().to_vec();

        let mut columns = left_columns.clone();
        columns.extend(right_columns.clone());

        Ok(Self {
            left,
            right,
            join_type,
            condition,
            _strategy: strategy,
            columns,
            left_columns,
            right_columns,
            right_data: None,
            left_batch: None,
            left_row_idx: 0,
            right_row_idx: 0,
        })
    }

    fn materialize_right(&mut self) -> ExecutorResult<()> {
        if self.right_data.is_some() {
            return Ok(());
        }

        let mut all_rows = Vec::new();
        while let Some(batch) = self.right.next_batch()? {
            all_rows.extend(batch.rows);
        }
        self.right_data = Some(all_rows);
        Ok(())
    }

    fn evaluate_join_condition(&self, left: &Row, right: &Row) -> ExecutorResult<bool> {
        match &self.condition {
            None => Ok(true),
            Some(expr) => {
                let mut combined = left.values.clone();
                combined.extend(right.values.clone());
                let combined_row = Row { values: combined };

                let value = evaluate_expression(expr, &combined_row, &self.columns)?;
                match value {
                    Value::Boolean(b) => Ok(b),
                    Value::Null => Ok(false),
                    _ => Ok(false),
                }
            }
        }
    }
}

impl<'a> Operator for JoinOperator<'a> {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        self.materialize_right()?;
        let right_data = self.right_data.as_ref().unwrap();

        let mut result_rows = Vec::new();

        loop {
            if self.left_batch.is_none() {
                self.left_batch = self.left.next_batch()?;
                self.left_row_idx = 0;
                self.right_row_idx = 0;

                if self.left_batch.is_none() {
                    break;
                }
            }

            let left_batch = self.left_batch.as_ref().unwrap();

            while self.left_row_idx < left_batch.rows.len() {
                let left_row = &left_batch.rows[self.left_row_idx];

                while self.right_row_idx < right_data.len() {
                    let right_row = &right_data[self.right_row_idx];
                    self.right_row_idx += 1;

                    if self.evaluate_join_condition(left_row, right_row)? {
                        let mut combined = left_row.values.clone();
                        combined.extend(right_row.values.clone());
                        result_rows.push(Row { values: combined });

                        if result_rows.len() >= 1024 {
                            return Ok(Some(ResultBatch::with_rows(
                                self.columns.clone(),
                                result_rows,
                            )));
                        }
                    }
                }

                self.left_row_idx += 1;
                self.right_row_idx = 0;
            }

            self.left_batch = None;
        }

        if result_rows.is_empty() {
            Ok(None)
        } else {
            Ok(Some(ResultBatch::with_rows(
                self.columns.clone(),
                result_rows,
            )))
        }
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Aggregate Operator
// =============================================================================

struct AggregateOperator<'a> {
    input: Box<dyn Operator + 'a>,
    _group_by: Vec<PlanExpression>,
    aggregates: Vec<crate::planner::AggregateExpr>,
    columns: Vec<String>,
    input_columns: Vec<String>,
    done: bool,
}

impl<'a> AggregateOperator<'a> {
    fn new(
        input: Box<dyn Operator + 'a>,
        group_by: Vec<PlanExpression>,
        aggregates: Vec<crate::planner::AggregateExpr>,
    ) -> Self {
        let input_columns = input.columns().to_vec();

        let columns: Vec<String> = aggregates
            .iter()
            .enumerate()
            .map(|(i, agg)| {
                agg.alias
                    .clone()
                    .unwrap_or_else(|| format!("agg_{}", i))
            })
            .collect();

        Self {
            input,
            _group_by: group_by,
            aggregates,
            columns,
            input_columns,
            done: false,
        }
    }
}

impl<'a> Operator for AggregateOperator<'a> {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        if self.done {
            return Ok(None);
        }

        let mut accumulators: Vec<Accumulator> = self
            .aggregates
            .iter()
            .map(|agg| Accumulator::new(agg.function))
            .collect();

        while let Some(batch) = self.input.next_batch()? {
            for row in &batch.rows {
                for (i, agg) in self.aggregates.iter().enumerate() {
                    let value = if let Some(ref arg) = agg.argument {
                        evaluate_expression(arg, row, &self.input_columns)?
                    } else {
                        Value::Integer(1)
                    };
                    accumulators[i].accumulate(&value)?;
                }
            }
        }

        let result_values: Vec<Value> = accumulators.iter().map(|acc| acc.finalize()).collect();

        self.done = true;

        Ok(Some(ResultBatch::with_rows(
            self.columns.clone(),
            vec![Row {
                values: result_values,
            }],
        )))
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

/// Accumulator for aggregate functions.
struct Accumulator {
    function: AggregateFunction,
    count: i64,
    sum: f64,
    min: Option<Value>,
    max: Option<Value>,
}

impl Accumulator {
    fn new(function: AggregateFunction) -> Self {
        Self {
            function,
            count: 0,
            sum: 0.0,
            min: None,
            max: None,
        }
    }

    fn accumulate(&mut self, value: &Value) -> ExecutorResult<()> {
        if matches!(value, Value::Null) {
            return Ok(());
        }

        self.count += 1;

        match self.function {
            AggregateFunction::Count => {}
            AggregateFunction::Sum | AggregateFunction::Avg => {
                self.sum += value_to_f64(value)?;
            }
            AggregateFunction::Min => {
                if self.min.is_none() || compare_values(value, self.min.as_ref().unwrap())? == std::cmp::Ordering::Less {
                    self.min = Some(value.clone());
                }
            }
            AggregateFunction::Max => {
                if self.max.is_none() || compare_values(value, self.max.as_ref().unwrap())? == std::cmp::Ordering::Greater {
                    self.max = Some(value.clone());
                }
            }
        }

        Ok(())
    }

    fn finalize(&self) -> Value {
        match self.function {
            AggregateFunction::Count => Value::Integer(self.count),
            AggregateFunction::Sum => Value::Float(self.sum),
            AggregateFunction::Avg => {
                if self.count == 0 {
                    Value::Null
                } else {
                    Value::Float(self.sum / self.count as f64)
                }
            }
            AggregateFunction::Min => self.min.clone().unwrap_or(Value::Null),
            AggregateFunction::Max => self.max.clone().unwrap_or(Value::Null),
        }
    }
}

// =============================================================================
// Sort Operator
// =============================================================================

struct SortOperator<'a> {
    input: Box<dyn Operator + 'a>,
    order_by: Vec<crate::planner::SortKey>,
    columns: Vec<String>,
    sorted_data: Option<Vec<Row>>,
    position: usize,
}

impl<'a> SortOperator<'a> {
    fn new(input: Box<dyn Operator + 'a>, order_by: Vec<crate::planner::SortKey>) -> Self {
        let columns = input.columns().to_vec();
        Self {
            input,
            order_by,
            columns,
            sorted_data: None,
            position: 0,
        }
    }
}

impl<'a> Operator for SortOperator<'a> {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        if self.sorted_data.is_none() {
            let mut all_rows = Vec::new();
            while let Some(batch) = self.input.next_batch()? {
                all_rows.extend(batch.rows);
            }

            let columns = self.columns.clone();
            let order_by = self.order_by.clone();

            all_rows.sort_by(|a, b| {
                for key in &order_by {
                    let a_val = evaluate_expression(&key.expr, a, &columns).unwrap_or(Value::Null);
                    let b_val = evaluate_expression(&key.expr, b, &columns).unwrap_or(Value::Null);

                    let cmp = compare_values(&a_val, &b_val).unwrap_or(std::cmp::Ordering::Equal);

                    if cmp != std::cmp::Ordering::Equal {
                        return if key.ascending {
                            cmp
                        } else {
                            cmp.reverse()
                        };
                    }
                }
                std::cmp::Ordering::Equal
            });

            self.sorted_data = Some(all_rows);
        }

        let data = self.sorted_data.as_ref().unwrap();

        if self.position >= data.len() {
            return Ok(None);
        }

        let end = (self.position + 1024).min(data.len());
        let rows = data[self.position..end].to_vec();
        self.position = end;

        Ok(Some(ResultBatch::with_rows(self.columns.clone(), rows)))
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Limit Operator
// =============================================================================

struct LimitOperator<'a> {
    input: Box<dyn Operator + 'a>,
    limit: Option<u64>,
    offset: Option<u64>,
    columns: Vec<String>,
    rows_skipped: u64,
    rows_returned: u64,
}

impl<'a> LimitOperator<'a> {
    fn new(input: Box<dyn Operator + 'a>, limit: Option<u64>, offset: Option<u64>) -> Self {
        let columns = input.columns().to_vec();
        Self {
            input,
            limit,
            offset,
            columns,
            rows_skipped: 0,
            rows_returned: 0,
        }
    }
}

impl<'a> Operator for LimitOperator<'a> {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        if let Some(limit) = self.limit {
            if self.rows_returned >= limit {
                return Ok(None);
            }
        }

        while let Some(batch) = self.input.next_batch()? {
            let mut rows = batch.rows;

            let offset = self.offset.unwrap_or(0);
            if self.rows_skipped < offset {
                let skip = (offset - self.rows_skipped) as usize;
                if skip >= rows.len() {
                    self.rows_skipped += rows.len() as u64;
                    continue;
                }
                rows = rows[skip..].to_vec();
                self.rows_skipped = offset;
            }

            if let Some(limit) = self.limit {
                let remaining = limit - self.rows_returned;
                if rows.len() as u64 > remaining {
                    rows.truncate(remaining as usize);
                }
            }

            if rows.is_empty() {
                continue;
            }

            self.rows_returned += rows.len() as u64;

            return Ok(Some(ResultBatch::with_rows(self.columns.clone(), rows)));
        }

        Ok(None)
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Empty Operator
// =============================================================================

struct EmptyOperator {
    done: bool,
}

impl EmptyOperator {
    fn new() -> Self {
        Self { done: false }
    }
}

impl Operator for EmptyOperator {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        if self.done {
            return Ok(None);
        }
        self.done = true;
        Ok(Some(ResultBatch::with_rows(vec![], vec![Row { values: vec![] }])))
    }

    fn columns(&self) -> &[String] {
        &[]
    }
}

// =============================================================================
// Expression Evaluation
// =============================================================================

/// Context for expression evaluation, optionally holding executor for subqueries.
pub struct EvalContext<'a> {
    executor: Option<&'a Executor>,
}

impl<'a> EvalContext<'a> {
    pub fn new() -> Self {
        Self { executor: None }
    }

    pub fn with_executor(executor: &'a Executor) -> Self {
        Self { executor: Some(executor) }
    }

    /// Execute a subquery and return all result rows.
    fn execute_subquery(&self, subquery: &PlanNode) -> ExecutorResult<Vec<Row>> {
        let executor = self.executor.ok_or_else(|| {
            ExecutorError::Internal("Subquery requires execution context".to_string())
        })?;
        let plan = QueryPlan {
            root: subquery.clone(),
            estimated_cost: 0.0,
            estimated_rows: 0,
        };
        let result = executor.execute(&plan)?;
        Ok(result.rows)
    }
}

impl Default for EvalContext<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluate a default expression (constant expression that doesn't need row context).
fn evaluate_default_expression(expr: &PlanExpression) -> ExecutorResult<Value> {
    let empty_row = Row { values: vec![] };
    let empty_columns: Vec<String> = vec![];
    evaluate_expression(expr, &empty_row, &empty_columns)
}

/// Substitute parameter placeholders in a query plan with actual values.
fn substitute_parameters(node: &PlanNode, params: &[Value]) -> ExecutorResult<PlanNode> {
    match node {
        PlanNode::Scan(scan) => {
            // ScanNode doesn't have predicates - predicates are in Filter nodes
            // Just need to handle index_scan key_range expressions if present
            let index_scan = if let Some(idx) = &scan.index_scan {
                let start = idx.key_range.start.as_ref()
                    .map(|e| substitute_expr(e, params))
                    .transpose()?;
                let end = idx.key_range.end.as_ref()
                    .map(|e| substitute_expr(e, params))
                    .transpose()?;
                Some(crate::planner::IndexScan {
                    index_name: idx.index_name.clone(),
                    key_range: crate::planner::KeyRange {
                        start,
                        start_inclusive: idx.key_range.start_inclusive,
                        end,
                        end_inclusive: idx.key_range.end_inclusive,
                    },
                })
            } else {
                None
            };
            Ok(PlanNode::Scan(ScanNode {
                table_name: scan.table_name.clone(),
                alias: scan.alias.clone(),
                columns: scan.columns.clone(),
                index_scan,
            }))
        }
        PlanNode::Filter(filter) => {
            let input = Box::new(substitute_parameters(&filter.input, params)?);
            let predicate = substitute_expr(&filter.predicate, params)?;
            Ok(PlanNode::Filter(FilterNode { input, predicate }))
        }
        PlanNode::Project(proj) => {
            let input = Box::new(substitute_parameters(&proj.input, params)?);
            let expressions = proj.expressions.iter()
                .map(|pe| Ok(ProjectionExpr {
                    expr: substitute_expr(&pe.expr, params)?,
                    alias: pe.alias.clone(),
                }))
                .collect::<ExecutorResult<Vec<_>>>()?;
            Ok(PlanNode::Project(ProjectNode { input, expressions }))
        }
        PlanNode::Sort(sort) => {
            let input = Box::new(substitute_parameters(&sort.input, params)?);
            let order_by = sort.order_by.iter()
                .map(|sk| Ok(SortKey {
                    expr: substitute_expr(&sk.expr, params)?,
                    ascending: sk.ascending,
                    nulls_first: sk.nulls_first,
                }))
                .collect::<ExecutorResult<Vec<_>>>()?;
            Ok(PlanNode::Sort(SortNode { input, order_by }))
        }
        PlanNode::Limit(limit) => {
            let input = Box::new(substitute_parameters(&limit.input, params)?);
            Ok(PlanNode::Limit(LimitNode {
                input,
                limit: limit.limit,
                offset: limit.offset,
            }))
        }
        PlanNode::Aggregate(agg) => {
            let input = Box::new(substitute_parameters(&agg.input, params)?);
            let group_by = agg.group_by.iter()
                .map(|e| substitute_expr(e, params))
                .collect::<ExecutorResult<Vec<_>>>()?;
            let aggregates = agg.aggregates.iter()
                .map(|ae| Ok(AggregateExpr {
                    function: ae.function,
                    argument: ae.argument.as_ref().map(|a| substitute_expr(a, params)).transpose()?,
                    distinct: ae.distinct,
                    alias: ae.alias.clone(),
                }))
                .collect::<ExecutorResult<Vec<_>>>()?;
            Ok(PlanNode::Aggregate(AggregateNode { input, group_by, aggregates }))
        }
        PlanNode::Join(join) => {
            let left = Box::new(substitute_parameters(&join.left, params)?);
            let right = Box::new(substitute_parameters(&join.right, params)?);
            let condition = join.condition.as_ref()
                .map(|c| substitute_expr(c, params))
                .transpose()?;
            Ok(PlanNode::Join(JoinNode {
                left,
                right,
                join_type: join.join_type,
                condition,
                strategy: join.strategy,
            }))
        }
        PlanNode::Insert(insert) => {
            let source = match &insert.source {
                InsertPlanSource::Values(rows) => {
                    let substituted = rows.iter()
                        .map(|row| row.iter().map(|e| substitute_expr(e, params)).collect::<ExecutorResult<Vec<_>>>())
                        .collect::<ExecutorResult<Vec<_>>>()?;
                    InsertPlanSource::Values(substituted)
                }
                InsertPlanSource::Query(q) => {
                    InsertPlanSource::Query(Box::new(substitute_parameters(q, params)?))
                }
            };
            Ok(PlanNode::Insert(InsertNode {
                table_name: insert.table_name.clone(),
                columns: insert.columns.clone(),
                source,
            }))
        }
        PlanNode::Update(update) => {
            let assignments = update.assignments.iter()
                .map(|(col, expr)| Ok((col.clone(), substitute_expr(expr, params)?)))
                .collect::<ExecutorResult<Vec<_>>>()?;
            let where_clause = update.where_clause.as_ref()
                .map(|p| substitute_expr(p, params))
                .transpose()?;
            Ok(PlanNode::Update(UpdateNode {
                table_name: update.table_name.clone(),
                assignments,
                where_clause,
            }))
        }
        PlanNode::Delete(delete) => {
            let where_clause = delete.where_clause.as_ref()
                .map(|p| substitute_expr(p, params))
                .transpose()?;
            Ok(PlanNode::Delete(DeleteNode {
                table_name: delete.table_name.clone(),
                where_clause,
            }))
        }
        // DDL operations don't have parameters, return as-is
        PlanNode::CreateTable(_) |
        PlanNode::DropTable(_) |
        PlanNode::AlterTable(_) |
        PlanNode::CreateIndex(_) |
        PlanNode::DropIndex(_) |
        PlanNode::Empty => Ok(node.clone()),
    }
}

/// Substitute parameter placeholders in an expression with actual values.
fn substitute_expr(expr: &PlanExpression, params: &[Value]) -> ExecutorResult<PlanExpression> {
    match expr {
        PlanExpression::Placeholder(idx) => {
            // Parameters are 1-indexed ($1, $2, etc.)
            let param_idx = *idx - 1;
            params.get(param_idx)
                .map(|v| value_to_literal(v))
                .ok_or_else(|| ExecutorError::Internal(format!(
                    "Missing parameter ${}: expected {} parameters but got {}",
                    idx, idx, params.len()
                )))
        }
        PlanExpression::BinaryOp { op, left, right } => {
            Ok(PlanExpression::BinaryOp {
                op: op.clone(),
                left: Box::new(substitute_expr(left, params)?),
                right: Box::new(substitute_expr(right, params)?),
            })
        }
        PlanExpression::UnaryOp { op, expr: inner } => {
            Ok(PlanExpression::UnaryOp {
                op: op.clone(),
                expr: Box::new(substitute_expr(inner, params)?),
            })
        }
        PlanExpression::Function { name, args, return_type } => {
            let new_args = args.iter()
                .map(|a| substitute_expr(a, params))
                .collect::<ExecutorResult<Vec<_>>>()?;
            Ok(PlanExpression::Function { name: name.clone(), args: new_args, return_type: return_type.clone() })
        }
        PlanExpression::Case { operand, conditions, else_result } => {
            let new_operand = operand.as_ref()
                .map(|o| substitute_expr(o, params))
                .transpose()?
                .map(Box::new);
            let new_conditions = conditions.iter()
                .map(|(cond, result)| Ok((substitute_expr(cond, params)?, substitute_expr(result, params)?)))
                .collect::<ExecutorResult<Vec<_>>>()?;
            let new_else = else_result.as_ref()
                .map(|e| substitute_expr(e, params))
                .transpose()?
                .map(Box::new);
            Ok(PlanExpression::Case {
                operand: new_operand,
                conditions: new_conditions,
                else_result: new_else,
            })
        }
        PlanExpression::InList { expr: inner, list, negated } => {
            let new_inner = Box::new(substitute_expr(inner, params)?);
            let new_list = list.iter()
                .map(|e| substitute_expr(e, params))
                .collect::<ExecutorResult<Vec<_>>>()?;
            Ok(PlanExpression::InList { expr: new_inner, list: new_list, negated: *negated })
        }
        PlanExpression::InSubquery { expr: inner, subquery, negated } => {
            let new_inner = Box::new(substitute_expr(inner, params)?);
            let new_subquery = Box::new(substitute_parameters(subquery, params)?);
            Ok(PlanExpression::InSubquery { expr: new_inner, subquery: new_subquery, negated: *negated })
        }
        PlanExpression::Exists { subquery, negated } => {
            let new_subquery = Box::new(substitute_parameters(subquery, params)?);
            Ok(PlanExpression::Exists { subquery: new_subquery, negated: *negated })
        }
        PlanExpression::ScalarSubquery(subquery) => {
            let new_subquery = Box::new(substitute_parameters(subquery, params)?);
            Ok(PlanExpression::ScalarSubquery(new_subquery))
        }
        PlanExpression::Between { expr: inner, low, high, negated } => {
            Ok(PlanExpression::Between {
                expr: Box::new(substitute_expr(inner, params)?),
                low: Box::new(substitute_expr(low, params)?),
                high: Box::new(substitute_expr(high, params)?),
                negated: *negated,
            })
        }
        PlanExpression::Like { expr: inner, pattern, negated } => {
            Ok(PlanExpression::Like {
                expr: Box::new(substitute_expr(inner, params)?),
                pattern: Box::new(substitute_expr(pattern, params)?),
                negated: *negated,
            })
        }
        PlanExpression::IsNull { expr: inner, negated } => {
            Ok(PlanExpression::IsNull {
                expr: Box::new(substitute_expr(inner, params)?),
                negated: *negated,
            })
        }
        PlanExpression::Cast { expr: inner, target_type } => {
            Ok(PlanExpression::Cast {
                expr: Box::new(substitute_expr(inner, params)?),
                target_type: target_type.clone(),
            })
        }
        // Literals and columns don't have parameters
        PlanExpression::Literal(_) | PlanExpression::Column { .. } => Ok(expr.clone()),
    }
}

/// Convert a Value back to a PlanExpression::Literal.
fn value_to_literal(value: &Value) -> PlanExpression {
    let lit = match value {
        Value::Null => PlanLiteral::Null,
        Value::Boolean(b) => PlanLiteral::Boolean(*b),
        Value::Integer(i) => PlanLiteral::Integer(*i),
        Value::Float(f) => PlanLiteral::Float(*f),
        Value::String(s) => PlanLiteral::String(s.clone()),
        Value::Bytes(b) => PlanLiteral::String(String::from_utf8_lossy(b).to_string()),
        Value::Timestamp(t) => PlanLiteral::Integer(t.timestamp_millis()),
        // Arrays and objects can be serialized to string representation for parameters
        Value::Array(arr) => PlanLiteral::String(format!("{:?}", arr)),
        Value::Object(obj) => PlanLiteral::String(format!("{:?}", obj)),
    };
    PlanExpression::Literal(lit)
}

/// Evaluate expression without subquery support (backwards compatible).
fn evaluate_expression(
    expr: &PlanExpression,
    row: &Row,
    columns: &[String],
) -> ExecutorResult<Value> {
    evaluate_expression_with_context(expr, row, columns, &EvalContext::new())
}

/// Evaluate expression with optional subquery support.
fn evaluate_expression_with_context(
    expr: &PlanExpression,
    row: &Row,
    columns: &[String],
    ctx: &EvalContext,
) -> ExecutorResult<Value> {
    match expr {
        PlanExpression::Literal(lit) => Ok(match lit {
            PlanLiteral::Null => Value::Null,
            PlanLiteral::Boolean(b) => Value::Boolean(*b),
            PlanLiteral::Integer(i) => Value::Integer(*i),
            PlanLiteral::Float(f) => Value::Float(*f),
            PlanLiteral::String(s) => Value::String(s.clone()),
        }),

        PlanExpression::Column { table: _, name, .. } => {
            if name == "*" {
                return Ok(Value::Null);
            }

            let idx = columns
                .iter()
                .position(|c| c == name)
                .ok_or_else(|| ExecutorError::ColumnNotFound(name.clone()))?;

            Ok(row.values.get(idx).cloned().unwrap_or(Value::Null))
        }

        PlanExpression::BinaryOp { left, op, right } => {
            let left_val = evaluate_expression_with_context(left, row, columns, ctx)?;
            let right_val = evaluate_expression_with_context(right, row, columns, ctx)?;
            evaluate_binary_op(*op, &left_val, &right_val)
        }

        PlanExpression::UnaryOp { op, expr } => {
            let val = evaluate_expression_with_context(expr, row, columns, ctx)?;
            evaluate_unary_op(*op, &val)
        }

        PlanExpression::IsNull { expr, negated } => {
            let val = evaluate_expression_with_context(expr, row, columns, ctx)?;
            let is_null = matches!(val, Value::Null);
            Ok(Value::Boolean(if *negated { !is_null } else { is_null }))
        }

        PlanExpression::Cast { expr, target_type } => {
            let val = evaluate_expression_with_context(expr, row, columns, ctx)?;
            cast_value(&val, target_type)
        }

        PlanExpression::Function { name, args, .. } => {
            let arg_values: Vec<Value> = args
                .iter()
                .map(|a| evaluate_expression_with_context(a, row, columns, ctx))
                .collect::<ExecutorResult<Vec<_>>>()?;
            evaluate_function(name, &arg_values)
        }

        PlanExpression::Case { operand, conditions, else_result } => {
            match operand {
                Some(operand_expr) => {
                    // CASE operand WHEN value THEN result ... END
                    let operand_val = evaluate_expression_with_context(operand_expr, row, columns, ctx)?;
                    for (when_expr, then_expr) in conditions {
                        let when_val = evaluate_expression_with_context(when_expr, row, columns, ctx)?;
                        if compare_values(&operand_val, &when_val)? == std::cmp::Ordering::Equal {
                            return evaluate_expression_with_context(then_expr, row, columns, ctx);
                        }
                    }
                }
                None => {
                    // CASE WHEN condition THEN result ... END
                    for (when_expr, then_expr) in conditions {
                        let when_val = evaluate_expression_with_context(when_expr, row, columns, ctx)?;
                        if matches!(when_val, Value::Boolean(true)) {
                            return evaluate_expression_with_context(then_expr, row, columns, ctx);
                        }
                    }
                }
            }
            // Return ELSE result or NULL
            match else_result {
                Some(else_expr) => evaluate_expression_with_context(else_expr, row, columns, ctx),
                None => Ok(Value::Null),
            }
        }

        PlanExpression::InList { expr, list, negated } => {
            let val = evaluate_expression_with_context(expr, row, columns, ctx)?;
            let mut found = false;
            for item in list {
                let item_val = evaluate_expression_with_context(item, row, columns, ctx)?;
                if compare_values(&val, &item_val)? == std::cmp::Ordering::Equal {
                    found = true;
                    break;
                }
            }
            Ok(Value::Boolean(if *negated { !found } else { found }))
        }

        PlanExpression::Between { expr, low, high, negated } => {
            let val = evaluate_expression_with_context(expr, row, columns, ctx)?;
            let low_val = evaluate_expression_with_context(low, row, columns, ctx)?;
            let high_val = evaluate_expression_with_context(high, row, columns, ctx)?;

            let ge_low = compare_values(&val, &low_val)? != std::cmp::Ordering::Less;
            let le_high = compare_values(&val, &high_val)? != std::cmp::Ordering::Greater;
            let in_range = ge_low && le_high;

            Ok(Value::Boolean(if *negated { !in_range } else { in_range }))
        }

        PlanExpression::Like { expr, pattern, negated } => {
            let val = evaluate_expression_with_context(expr, row, columns, ctx)?;
            let pattern_val = evaluate_expression_with_context(pattern, row, columns, ctx)?;

            let val_str = match val {
                Value::String(s) => s,
                Value::Null => return Ok(Value::Null),
                Value::Integer(i) => i.to_string(),
                Value::Float(f) => f.to_string(),
                Value::Boolean(b) => b.to_string(),
                Value::Bytes(b) => String::from_utf8_lossy(&b).to_string(),
                Value::Timestamp(t) => t.to_rfc3339(),
                Value::Array(_) | Value::Object(_) => return Ok(Value::Boolean(false)),
            };

            let pattern_str = match pattern_val {
                Value::String(s) => s,
                Value::Null => return Ok(Value::Null),
                Value::Integer(i) => i.to_string(),
                Value::Float(f) => f.to_string(),
                Value::Boolean(b) => b.to_string(),
                Value::Bytes(b) => String::from_utf8_lossy(&b).to_string(),
                Value::Timestamp(t) => t.to_rfc3339(),
                Value::Array(_) | Value::Object(_) => return Ok(Value::Boolean(false)),
            };

            // Convert SQL LIKE pattern to regex
            let regex_pattern = pattern_str
                .replace('%', ".*")
                .replace('_', ".");
            let regex_pattern = format!("^{}$", regex_pattern);

            // Simple pattern matching (could use regex crate for full support)
            let matches = if regex_pattern == "^.*$" {
                true
            } else if regex_pattern.starts_with("^") && regex_pattern.ends_with("$") {
                let inner = &regex_pattern[1..regex_pattern.len()-1];
                if inner.contains(".*") || inner.contains('.') {
                    // For patterns with wildcards, do simple matching
                    let parts: Vec<&str> = inner.split(".*").collect();
                    if parts.len() == 1 {
                        val_str == parts[0]
                    } else {
                        let mut pos = 0;
                        let mut matched = true;
                        for (i, part) in parts.iter().enumerate() {
                            if part.is_empty() { continue; }
                            if let Some(found_pos) = val_str[pos..].find(part) {
                                if i == 0 && found_pos != 0 {
                                    matched = false;
                                    break;
                                }
                                pos += found_pos + part.len();
                            } else {
                                matched = false;
                                break;
                            }
                        }
                        matched
                    }
                } else {
                    val_str == inner
                }
            } else {
                val_str.contains(&pattern_str)
            };

            Ok(Value::Boolean(if *negated { !matches } else { matches }))
        }

        PlanExpression::InSubquery { expr, subquery, negated } => {
            // Evaluate the expression to check
            let val = evaluate_expression_with_context(expr, row, columns, ctx)?;

            // Execute the subquery
            let subquery_rows = ctx.execute_subquery(subquery)?;

            // Check if val is in any of the subquery results (first column)
            let mut found = false;
            for subquery_row in &subquery_rows {
                if let Some(subquery_val) = subquery_row.values.first() {
                    if compare_values(&val, subquery_val)? == std::cmp::Ordering::Equal {
                        found = true;
                        break;
                    }
                }
            }

            Ok(Value::Boolean(if *negated { !found } else { found }))
        }

        PlanExpression::Exists { subquery, negated } => {
            // Execute the subquery and check if it returns any rows
            let subquery_rows = ctx.execute_subquery(subquery)?;
            let exists = !subquery_rows.is_empty();

            Ok(Value::Boolean(if *negated { !exists } else { exists }))
        }

        PlanExpression::ScalarSubquery(subquery) => {
            // Execute the subquery and return the single value
            let subquery_rows = ctx.execute_subquery(subquery)?;

            if subquery_rows.is_empty() {
                return Ok(Value::Null);
            }

            if subquery_rows.len() > 1 {
                return Err(ExecutorError::Internal(
                    "Scalar subquery returned more than one row".to_string()
                ));
            }

            // Return the first column of the first row
            subquery_rows[0]
                .values
                .first()
                .cloned()
                .ok_or_else(|| ExecutorError::Internal("Scalar subquery returned no columns".to_string()))
        }

        PlanExpression::Placeholder(idx) => {
            // Placeholders should be replaced before execution
            Err(ExecutorError::Internal(format!("Unresolved placeholder ${}", idx)))
        }
    }
}

fn evaluate_binary_op(op: PlanBinaryOp, left: &Value, right: &Value) -> ExecutorResult<Value> {
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        if matches!(op, PlanBinaryOp::Equal | PlanBinaryOp::NotEqual) {
            return Ok(Value::Null);
        }
        return Ok(Value::Null);
    }

    match op {
        PlanBinaryOp::Add => {
            let l = value_to_f64(left)?;
            let r = value_to_f64(right)?;
            Ok(Value::Float(l + r))
        }
        PlanBinaryOp::Subtract => {
            let l = value_to_f64(left)?;
            let r = value_to_f64(right)?;
            Ok(Value::Float(l - r))
        }
        PlanBinaryOp::Multiply => {
            let l = value_to_f64(left)?;
            let r = value_to_f64(right)?;
            Ok(Value::Float(l * r))
        }
        PlanBinaryOp::Divide => {
            let l = value_to_f64(left)?;
            let r = value_to_f64(right)?;
            if r == 0.0 {
                return Err(ExecutorError::DivisionByZero);
            }
            Ok(Value::Float(l / r))
        }
        PlanBinaryOp::Modulo => {
            let l = value_to_i64(left)?;
            let r = value_to_i64(right)?;
            if r == 0 {
                return Err(ExecutorError::DivisionByZero);
            }
            Ok(Value::Integer(l % r))
        }
        PlanBinaryOp::Equal => Ok(Value::Boolean(compare_values(left, right)? == std::cmp::Ordering::Equal)),
        PlanBinaryOp::NotEqual => Ok(Value::Boolean(compare_values(left, right)? != std::cmp::Ordering::Equal)),
        PlanBinaryOp::LessThan => Ok(Value::Boolean(compare_values(left, right)? == std::cmp::Ordering::Less)),
        PlanBinaryOp::LessThanOrEqual => Ok(Value::Boolean(compare_values(left, right)? != std::cmp::Ordering::Greater)),
        PlanBinaryOp::GreaterThan => Ok(Value::Boolean(compare_values(left, right)? == std::cmp::Ordering::Greater)),
        PlanBinaryOp::GreaterThanOrEqual => Ok(Value::Boolean(compare_values(left, right)? != std::cmp::Ordering::Less)),
        PlanBinaryOp::And => {
            let l = value_to_bool(left)?;
            let r = value_to_bool(right)?;
            Ok(Value::Boolean(l && r))
        }
        PlanBinaryOp::Or => {
            let l = value_to_bool(left)?;
            let r = value_to_bool(right)?;
            Ok(Value::Boolean(l || r))
        }
        PlanBinaryOp::Concat => {
            let l = value_to_string(left);
            let r = value_to_string(right);
            Ok(Value::String(format!("{}{}", l, r)))
        }
    }
}

fn evaluate_unary_op(op: PlanUnaryOp, value: &Value) -> ExecutorResult<Value> {
    match op {
        PlanUnaryOp::Not => {
            let b = value_to_bool(value)?;
            Ok(Value::Boolean(!b))
        }
        PlanUnaryOp::Negative => {
            let f = value_to_f64(value)?;
            Ok(Value::Float(-f))
        }
    }
}

fn evaluate_function(name: &str, args: &[Value]) -> ExecutorResult<Value> {
    match name.to_uppercase().as_str() {
        "UPPER" => {
            let s = value_to_string(&args.first().cloned().unwrap_or(Value::Null));
            Ok(Value::String(s.to_uppercase()))
        }
        "LOWER" => {
            let s = value_to_string(&args.first().cloned().unwrap_or(Value::Null));
            Ok(Value::String(s.to_lowercase()))
        }
        "LENGTH" => {
            let s = value_to_string(&args.first().cloned().unwrap_or(Value::Null));
            Ok(Value::Integer(s.len() as i64))
        }
        "ABS" => {
            let f = value_to_f64(&args.first().cloned().unwrap_or(Value::Null))?;
            Ok(Value::Float(f.abs()))
        }
        "COALESCE" => {
            for arg in args {
                if !matches!(arg, Value::Null) {
                    return Ok(arg.clone());
                }
            }
            Ok(Value::Null)
        }
        _ => Err(ExecutorError::InvalidOperation(format!(
            "Unknown function: {}",
            name
        ))),
    }
}

// =============================================================================
// Value Conversion Utilities
// =============================================================================

fn value_to_f64(value: &Value) -> ExecutorResult<f64> {
    match value {
        Value::Integer(i) => Ok(*i as f64),
        Value::Float(f) => Ok(*f),
        Value::String(s) => s.parse().map_err(|_| ExecutorError::TypeMismatch {
            expected: "number".to_string(),
            actual: "string".to_string(),
        }),
        _ => Err(ExecutorError::TypeMismatch {
            expected: "number".to_string(),
            actual: format!("{:?}", value),
        }),
    }
}

fn value_to_i64(value: &Value) -> ExecutorResult<i64> {
    match value {
        Value::Integer(i) => Ok(*i),
        Value::Float(f) => Ok(*f as i64),
        Value::String(s) => s.parse().map_err(|_| ExecutorError::TypeMismatch {
            expected: "integer".to_string(),
            actual: "string".to_string(),
        }),
        _ => Err(ExecutorError::TypeMismatch {
            expected: "integer".to_string(),
            actual: format!("{:?}", value),
        }),
    }
}

fn value_to_bool(value: &Value) -> ExecutorResult<bool> {
    match value {
        Value::Boolean(b) => Ok(*b),
        Value::Integer(i) => Ok(*i != 0),
        Value::Null => Ok(false),
        _ => Err(ExecutorError::TypeMismatch {
            expected: "boolean".to_string(),
            actual: format!("{:?}", value),
        }),
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Null => String::new(),
        _ => format!("{:?}", value),
    }
}

fn compare_values(left: &Value, right: &Value) -> ExecutorResult<std::cmp::Ordering> {
    match (left, right) {
        (Value::Integer(l), Value::Integer(r)) => Ok(l.cmp(r)),
        (Value::Float(l), Value::Float(r)) => {
            Ok(l.partial_cmp(r).unwrap_or(std::cmp::Ordering::Equal))
        }
        (Value::Integer(l), Value::Float(r)) => {
            let l = *l as f64;
            Ok(l.partial_cmp(r).unwrap_or(std::cmp::Ordering::Equal))
        }
        (Value::Float(l), Value::Integer(r)) => {
            let r = *r as f64;
            Ok(l.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal))
        }
        (Value::String(l), Value::String(r)) => Ok(l.cmp(r)),
        (Value::Boolean(l), Value::Boolean(r)) => Ok(l.cmp(r)),
        _ => Ok(std::cmp::Ordering::Equal),
    }
}

fn cast_value(value: &Value, target_type: &DataType) -> ExecutorResult<Value> {
    match target_type {
        DataType::Integer => Ok(Value::Integer(value_to_i64(value)?)),
        DataType::Float => Ok(Value::Float(value_to_f64(value)?)),
        DataType::Text => Ok(Value::String(value_to_string(value))),
        DataType::Boolean => Ok(Value::Boolean(value_to_bool(value)?)),
        _ => Ok(value.clone()),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{LimitNode, PlanNode, ProjectNode, ProjectionExpr, QueryPlan, ScanNode};

    fn create_test_context() -> ExecutionContext {
        let mut context = ExecutionContext::new();

        context.add_table(TableData {
            name: "users".to_string(),
            columns: vec![
                "id".to_string(),
                "name".to_string(),
                "age".to_string(),
            ],
            rows: vec![
                Row {
                    values: vec![
                        Value::Integer(1),
                        Value::String("Alice".to_string()),
                        Value::Integer(30),
                    ],
                },
                Row {
                    values: vec![
                        Value::Integer(2),
                        Value::String("Bob".to_string()),
                        Value::Integer(25),
                    ],
                },
                Row {
                    values: vec![
                        Value::Integer(3),
                        Value::String("Charlie".to_string()),
                        Value::Integer(35),
                    ],
                },
            ],
        });

        context
    }

    #[test]
    fn test_scan_operator() {
        let context = create_test_context();
        let executor = Executor::new(context);

        let plan = QueryPlan {
            root: PlanNode::Project(ProjectNode {
                input: Box::new(PlanNode::Scan(ScanNode {
                    table_name: "users".to_string(),
                    alias: None,
                    columns: vec![
                        "id".to_string(),
                        "name".to_string(),
                        "age".to_string(),
                    ],
                    index_scan: None,
                })),
                expressions: vec![
                    ProjectionExpr {
                        expr: PlanExpression::Column {
                            table: None,
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                        },
                        alias: Some("id".to_string()),
                    },
                    ProjectionExpr {
                        expr: PlanExpression::Column {
                            table: None,
                            name: "name".to_string(),
                            data_type: DataType::Text,
                        },
                        alias: Some("name".to_string()),
                    },
                ],
            }),
            estimated_cost: 100.0,
            estimated_rows: 3,
        };

        let result = executor.execute(&plan).unwrap();

        assert_eq!(result.rows.len(), 3);
        assert_eq!(result.columns.len(), 2);
    }

    #[test]
    fn test_filter_operator() {
        let context = create_test_context();
        let executor = Executor::new(context);

        let plan = QueryPlan {
            root: PlanNode::Project(ProjectNode {
                input: Box::new(PlanNode::Filter(crate::planner::FilterNode {
                    input: Box::new(PlanNode::Scan(ScanNode {
                        table_name: "users".to_string(),
                        alias: None,
                        columns: vec![
                            "id".to_string(),
                            "name".to_string(),
                            "age".to_string(),
                        ],
                        index_scan: None,
                    })),
                    predicate: PlanExpression::BinaryOp {
                        left: Box::new(PlanExpression::Column {
                            table: None,
                            name: "age".to_string(),
                            data_type: DataType::Integer,
                        }),
                        op: PlanBinaryOp::GreaterThan,
                        right: Box::new(PlanExpression::Literal(PlanLiteral::Integer(28))),
                    },
                })),
                expressions: vec![ProjectionExpr {
                    expr: PlanExpression::Column {
                        table: None,
                        name: "name".to_string(),
                        data_type: DataType::Text,
                    },
                    alias: Some("name".to_string()),
                }],
            }),
            estimated_cost: 100.0,
            estimated_rows: 2,
        };

        let result = executor.execute(&plan).unwrap();

        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_limit_operator() {
        let context = create_test_context();
        let executor = Executor::new(context);

        let plan = QueryPlan {
            root: PlanNode::Limit(LimitNode {
                input: Box::new(PlanNode::Project(ProjectNode {
                    input: Box::new(PlanNode::Scan(ScanNode {
                        table_name: "users".to_string(),
                        alias: None,
                        columns: vec!["id".to_string()],
                        index_scan: None,
                    })),
                    expressions: vec![ProjectionExpr {
                        expr: PlanExpression::Column {
                            table: None,
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                        },
                        alias: Some("id".to_string()),
                    }],
                })),
                limit: Some(2),
                offset: None,
            }),
            estimated_cost: 100.0,
            estimated_rows: 2,
        };

        let result = executor.execute(&plan).unwrap();

        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_create_table() {
        use crate::planner::{CreateTableNode, CreateColumnDef};

        let executor = Executor::new(ExecutionContext::new());

        let plan = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "test_table".to_string(),
                columns: vec![
                    CreateColumnDef {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        primary_key: true,
                        unique: false,
                    },
                    CreateColumnDef {
                        name: "name".to_string(),
                        data_type: DataType::Text,
                        nullable: true,
                        default: None,
                        primary_key: false,
                        unique: false,
                    },
                ],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };

        let result = executor.execute(&plan).unwrap();
        match &result.rows[0].values[0] {
            Value::String(s) => assert!(s.contains("created")),
            _ => panic!("Expected string result"),
        }

        // Verify table exists by listing tables
        let tables = executor.context.read().unwrap().list_tables();
        assert!(tables.contains(&"test_table".to_string()));
    }

    #[test]
    fn test_insert_into_table() {
        use crate::planner::{CreateTableNode, CreateColumnDef, InsertNode, InsertPlanSource};

        let executor = Executor::new(ExecutionContext::new());

        // First create a table
        let create_plan = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "test_insert".to_string(),
                columns: vec![
                    CreateColumnDef {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        primary_key: true,
                        unique: false,
                    },
                    CreateColumnDef {
                        name: "value".to_string(),
                        data_type: DataType::Text,
                        nullable: true,
                        default: None,
                        primary_key: false,
                        unique: false,
                    },
                ],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_plan).unwrap();

        // Now insert data
        let insert_plan = QueryPlan {
            root: PlanNode::Insert(InsertNode {
                table_name: "test_insert".to_string(),
                columns: vec!["id".to_string(), "value".to_string()],
                source: InsertPlanSource::Values(vec![
                    vec![
                        PlanExpression::Literal(PlanLiteral::Integer(1)),
                        PlanExpression::Literal(PlanLiteral::String("hello".to_string())),
                    ],
                    vec![
                        PlanExpression::Literal(PlanLiteral::Integer(2)),
                        PlanExpression::Literal(PlanLiteral::String("world".to_string())),
                    ],
                ]),
            }),
            estimated_cost: 2.0,
            estimated_rows: 2,
        };

        let result = executor.execute(&insert_plan).unwrap();
        assert_eq!(result.rows_affected, 2);

        // Query the data
        let query_plan = QueryPlan {
            root: PlanNode::Project(ProjectNode {
                input: Box::new(PlanNode::Scan(ScanNode {
                    table_name: "test_insert".to_string(),
                    alias: None,
                    columns: vec!["id".to_string(), "value".to_string()],
                    index_scan: None,
                })),
                expressions: vec![
                    ProjectionExpr {
                        expr: PlanExpression::Column {
                            table: None,
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                        },
                        alias: Some("id".to_string()),
                    },
                ],
            }),
            estimated_cost: 100.0,
            estimated_rows: 2,
        };

        let query_result = executor.execute(&query_plan).unwrap();
        assert_eq!(query_result.rows.len(), 2);
    }

    #[test]
    fn test_drop_table() {
        use crate::planner::{CreateTableNode, CreateColumnDef, DropTableNode};

        let executor = Executor::new(ExecutionContext::new());

        // Create a table
        let create_plan = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "to_drop".to_string(),
                columns: vec![CreateColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                    default: None,
                    primary_key: true,
                    unique: false,
                }],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_plan).unwrap();

        // Verify it exists
        let tables = executor.context.read().unwrap().list_tables();
        assert!(tables.contains(&"to_drop".to_string()));

        // Drop the table
        let drop_plan = QueryPlan {
            root: PlanNode::DropTable(DropTableNode {
                table_name: "to_drop".to_string(),
                if_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&drop_plan).unwrap();

        // Verify it's gone
        let tables = executor.context.read().unwrap().list_tables();
        assert!(!tables.contains(&"to_drop".to_string()));
    }

    #[test]
    fn test_shared_context_persistence() {
        use crate::planner::{CreateTableNode, CreateColumnDef};
        use std::sync::{Arc, RwLock};

        // Create a shared context
        let shared_context = Arc::new(RwLock::new(ExecutionContext::new()));

        // Create executor with shared context
        let executor = Executor::with_shared_context(shared_context.clone());

        // Create a table
        let create_plan = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "shared_table".to_string(),
                columns: vec![CreateColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                    default: None,
                    primary_key: true,
                    unique: false,
                }],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_plan).unwrap();

        // Create a new executor with the SAME shared context
        let executor2 = Executor::with_shared_context(shared_context.clone());

        // Verify the table exists from the second executor's perspective
        let tables = executor2.context.read().unwrap().list_tables();
        assert!(tables.contains(&"shared_table".to_string()));
    }

    #[test]
    fn test_insert_select() {
        use crate::planner::{CreateTableNode, CreateColumnDef, InsertNode, InsertPlanSource, FilterNode};

        let executor = Executor::new(ExecutionContext::new());

        // Create source table
        let create_source = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "source_table".to_string(),
                columns: vec![
                    CreateColumnDef {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        primary_key: true,
                        unique: false,
                    },
                    CreateColumnDef {
                        name: "value".to_string(),
                        data_type: DataType::Text,
                        nullable: true,
                        default: None,
                        primary_key: false,
                        unique: false,
                    },
                ],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_source).unwrap();

        // Create destination table
        let create_dest = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "dest_table".to_string(),
                columns: vec![
                    CreateColumnDef {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        primary_key: true,
                        unique: false,
                    },
                    CreateColumnDef {
                        name: "value".to_string(),
                        data_type: DataType::Text,
                        nullable: true,
                        default: None,
                        primary_key: false,
                        unique: false,
                    },
                ],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_dest).unwrap();

        // Insert data into source table
        let insert_source = QueryPlan {
            root: PlanNode::Insert(InsertNode {
                table_name: "source_table".to_string(),
                columns: vec!["id".to_string(), "value".to_string()],
                source: InsertPlanSource::Values(vec![
                    vec![
                        PlanExpression::Literal(PlanLiteral::Integer(1)),
                        PlanExpression::Literal(PlanLiteral::String("one".to_string())),
                    ],
                    vec![
                        PlanExpression::Literal(PlanLiteral::Integer(2)),
                        PlanExpression::Literal(PlanLiteral::String("two".to_string())),
                    ],
                    vec![
                        PlanExpression::Literal(PlanLiteral::Integer(3)),
                        PlanExpression::Literal(PlanLiteral::String("three".to_string())),
                    ],
                ]),
            }),
            estimated_cost: 3.0,
            estimated_rows: 3,
        };
        executor.execute(&insert_source).unwrap();

        // INSERT INTO dest_table SELECT * FROM source_table WHERE id > 1
        let insert_select = QueryPlan {
            root: PlanNode::Insert(InsertNode {
                table_name: "dest_table".to_string(),
                columns: vec!["id".to_string(), "value".to_string()],
                source: InsertPlanSource::Query(Box::new(
                    PlanNode::Project(ProjectNode {
                        input: Box::new(PlanNode::Filter(FilterNode {
                            input: Box::new(PlanNode::Scan(ScanNode {
                                table_name: "source_table".to_string(),
                                alias: None,
                                columns: vec!["id".to_string(), "value".to_string()],
                                index_scan: None,
                            })),
                            predicate: PlanExpression::BinaryOp {
                                left: Box::new(PlanExpression::Column {
                                    table: None,
                                    name: "id".to_string(),
                                    data_type: DataType::Integer,
                                }),
                                op: PlanBinaryOp::GreaterThan,
                                right: Box::new(PlanExpression::Literal(PlanLiteral::Integer(1))),
                            },
                        })),
                        expressions: vec![
                            ProjectionExpr {
                                expr: PlanExpression::Column {
                                    table: None,
                                    name: "id".to_string(),
                                    data_type: DataType::Integer,
                                },
                                alias: Some("id".to_string()),
                            },
                            ProjectionExpr {
                                expr: PlanExpression::Column {
                                    table: None,
                                    name: "value".to_string(),
                                    data_type: DataType::Text,
                                },
                                alias: Some("value".to_string()),
                            },
                        ],
                    })
                )),
            }),
            estimated_cost: 2.0,
            estimated_rows: 2,
        };

        let result = executor.execute(&insert_select).unwrap();
        assert_eq!(result.rows_affected, 2); // Only rows with id > 1 (id=2 and id=3)

        // Query dest_table to verify
        let query_plan = QueryPlan {
            root: PlanNode::Project(ProjectNode {
                input: Box::new(PlanNode::Scan(ScanNode {
                    table_name: "dest_table".to_string(),
                    alias: None,
                    columns: vec!["id".to_string(), "value".to_string()],
                    index_scan: None,
                })),
                expressions: vec![
                    ProjectionExpr {
                        expr: PlanExpression::Column {
                            table: None,
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                        },
                        alias: Some("id".to_string()),
                    },
                ],
            }),
            estimated_cost: 100.0,
            estimated_rows: 2,
        };

        let query_result = executor.execute(&query_plan).unwrap();
        assert_eq!(query_result.rows.len(), 2);
    }

    #[test]
    fn test_case_expression() {
        let row = Row { values: vec![Value::Integer(2)] };
        let columns = vec!["status".to_string()];

        // CASE WHEN status = 1 THEN 'one' WHEN status = 2 THEN 'two' ELSE 'other' END
        let case_expr = PlanExpression::Case {
            operand: None,
            conditions: vec![
                (
                    PlanExpression::BinaryOp {
                        left: Box::new(PlanExpression::Column {
                            table: None,
                            name: "status".to_string(),
                            data_type: DataType::Integer,
                        }),
                        op: PlanBinaryOp::Equal,
                        right: Box::new(PlanExpression::Literal(PlanLiteral::Integer(1))),
                    },
                    PlanExpression::Literal(PlanLiteral::String("one".to_string())),
                ),
                (
                    PlanExpression::BinaryOp {
                        left: Box::new(PlanExpression::Column {
                            table: None,
                            name: "status".to_string(),
                            data_type: DataType::Integer,
                        }),
                        op: PlanBinaryOp::Equal,
                        right: Box::new(PlanExpression::Literal(PlanLiteral::Integer(2))),
                    },
                    PlanExpression::Literal(PlanLiteral::String("two".to_string())),
                ),
            ],
            else_result: Some(Box::new(PlanExpression::Literal(PlanLiteral::String("other".to_string())))),
        };

        let result = evaluate_expression(&case_expr, &row, &columns).unwrap();
        assert_eq!(result, Value::String("two".to_string()));
    }

    #[test]
    fn test_in_list_expression() {
        let row = Row { values: vec![Value::Integer(3)] };
        let columns = vec!["id".to_string()];

        // id IN (1, 2, 3, 4, 5)
        let in_expr = PlanExpression::InList {
            expr: Box::new(PlanExpression::Column {
                table: None,
                name: "id".to_string(),
                data_type: DataType::Integer,
            }),
            list: vec![
                PlanExpression::Literal(PlanLiteral::Integer(1)),
                PlanExpression::Literal(PlanLiteral::Integer(2)),
                PlanExpression::Literal(PlanLiteral::Integer(3)),
                PlanExpression::Literal(PlanLiteral::Integer(4)),
                PlanExpression::Literal(PlanLiteral::Integer(5)),
            ],
            negated: false,
        };

        let result = evaluate_expression(&in_expr, &row, &columns).unwrap();
        assert_eq!(result, Value::Boolean(true));

        // id NOT IN (1, 2, 3, 4, 5) - should be false
        let not_in_expr = PlanExpression::InList {
            expr: Box::new(PlanExpression::Column {
                table: None,
                name: "id".to_string(),
                data_type: DataType::Integer,
            }),
            list: vec![
                PlanExpression::Literal(PlanLiteral::Integer(1)),
                PlanExpression::Literal(PlanLiteral::Integer(2)),
                PlanExpression::Literal(PlanLiteral::Integer(3)),
            ],
            negated: true,
        };

        let result = evaluate_expression(&not_in_expr, &row, &columns).unwrap();
        assert_eq!(result, Value::Boolean(false));
    }

    #[test]
    fn test_between_expression() {
        let row = Row { values: vec![Value::Integer(50)] };
        let columns = vec!["value".to_string()];

        // value BETWEEN 10 AND 100
        let between_expr = PlanExpression::Between {
            expr: Box::new(PlanExpression::Column {
                table: None,
                name: "value".to_string(),
                data_type: DataType::Integer,
            }),
            low: Box::new(PlanExpression::Literal(PlanLiteral::Integer(10))),
            high: Box::new(PlanExpression::Literal(PlanLiteral::Integer(100))),
            negated: false,
        };

        let result = evaluate_expression(&between_expr, &row, &columns).unwrap();
        assert_eq!(result, Value::Boolean(true));

        // value NOT BETWEEN 10 AND 40 (50 is outside, should be true)
        let not_between_expr = PlanExpression::Between {
            expr: Box::new(PlanExpression::Column {
                table: None,
                name: "value".to_string(),
                data_type: DataType::Integer,
            }),
            low: Box::new(PlanExpression::Literal(PlanLiteral::Integer(10))),
            high: Box::new(PlanExpression::Literal(PlanLiteral::Integer(40))),
            negated: true,
        };

        let result = evaluate_expression(&not_between_expr, &row, &columns).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_like_expression() {
        let row = Row { values: vec![Value::String("hello world".to_string())] };
        let columns = vec!["text".to_string()];

        // text LIKE 'hello%'
        let like_expr = PlanExpression::Like {
            expr: Box::new(PlanExpression::Column {
                table: None,
                name: "text".to_string(),
                data_type: DataType::Text,
            }),
            pattern: Box::new(PlanExpression::Literal(PlanLiteral::String("hello%".to_string()))),
            negated: false,
        };

        let result = evaluate_expression(&like_expr, &row, &columns).unwrap();
        assert_eq!(result, Value::Boolean(true));

        // text LIKE '%world'
        let like_expr2 = PlanExpression::Like {
            expr: Box::new(PlanExpression::Column {
                table: None,
                name: "text".to_string(),
                data_type: DataType::Text,
            }),
            pattern: Box::new(PlanExpression::Literal(PlanLiteral::String("%world".to_string()))),
            negated: false,
        };

        let result = evaluate_expression(&like_expr2, &row, &columns).unwrap();
        assert_eq!(result, Value::Boolean(true));

        // text NOT LIKE '%foo%' (should be true since 'foo' is not in 'hello world')
        let not_like_expr = PlanExpression::Like {
            expr: Box::new(PlanExpression::Column {
                table: None,
                name: "text".to_string(),
                data_type: DataType::Text,
            }),
            pattern: Box::new(PlanExpression::Literal(PlanLiteral::String("%foo%".to_string()))),
            negated: true,
        };

        let result = evaluate_expression(&not_like_expr, &row, &columns).unwrap();
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_alter_table() {
        use crate::planner::{CreateTableNode, CreateColumnDef, AlterTableNode, PlanAlterOperation};

        let executor = Executor::new(ExecutionContext::new());

        // Create a table
        let create_plan = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "alter_test".to_string(),
                columns: vec![
                    CreateColumnDef {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        primary_key: true,
                        unique: false,
                    },
                ],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_plan).unwrap();

        // Add a column
        let add_column_plan = QueryPlan {
            root: PlanNode::AlterTable(AlterTableNode {
                table_name: "alter_test".to_string(),
                operations: vec![
                    PlanAlterOperation::AddColumn(CreateColumnDef {
                        name: "name".to_string(),
                        data_type: DataType::Text,
                        nullable: true,
                        default: None,
                        primary_key: false,
                        unique: false,
                    }),
                ],
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        let result = executor.execute(&add_column_plan).unwrap();
        match &result.rows[0].values[0] {
            Value::String(s) => assert!(s.contains("altered")),
            _ => panic!("Expected string result"),
        }

        // Verify the column was added
        let schema = executor.context.read().unwrap().get_table_schema("alter_test").unwrap().clone();
        assert_eq!(schema.columns.len(), 2);
        assert_eq!(schema.columns[1].name, "name");

        // Rename the column
        let rename_column_plan = QueryPlan {
            root: PlanNode::AlterTable(AlterTableNode {
                table_name: "alter_test".to_string(),
                operations: vec![
                    PlanAlterOperation::RenameColumn {
                        old_name: "name".to_string(),
                        new_name: "full_name".to_string(),
                    },
                ],
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&rename_column_plan).unwrap();

        // Verify the column was renamed
        let schema = executor.context.read().unwrap().get_table_schema("alter_test").unwrap().clone();
        assert_eq!(schema.columns[1].name, "full_name");

        // Drop the column
        let drop_column_plan = QueryPlan {
            root: PlanNode::AlterTable(AlterTableNode {
                table_name: "alter_test".to_string(),
                operations: vec![
                    PlanAlterOperation::DropColumn {
                        name: "full_name".to_string(),
                        if_exists: false,
                    },
                ],
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&drop_column_plan).unwrap();

        // Verify the column was dropped
        let schema = executor.context.read().unwrap().get_table_schema("alter_test").unwrap().clone();
        assert_eq!(schema.columns.len(), 1);
    }

    #[test]
    fn test_alter_table_constraints() {
        use crate::planner::{CreateTableNode, CreateColumnDef, AlterTableNode, PlanAlterOperation, CreateTableConstraint};

        let executor = Executor::new(ExecutionContext::new());

        // Create a table with two columns
        let create_plan = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "constraint_test".to_string(),
                columns: vec![
                    CreateColumnDef {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        primary_key: false,
                        unique: false,
                    },
                    CreateColumnDef {
                        name: "email".to_string(),
                        data_type: DataType::Text,
                        nullable: true,
                        default: None,
                        primary_key: false,
                        unique: false,
                    },
                ],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_plan).unwrap();

        // Add a UNIQUE constraint
        let add_unique_plan = QueryPlan {
            root: PlanNode::AlterTable(AlterTableNode {
                table_name: "constraint_test".to_string(),
                operations: vec![
                    PlanAlterOperation::AddConstraint(CreateTableConstraint::Unique {
                        columns: vec!["email".to_string()],
                    }),
                ],
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&add_unique_plan).unwrap();

        // Verify the constraint was added
        let schema = executor.context.read().unwrap().get_table_schema("constraint_test").unwrap().clone();
        assert_eq!(schema.constraints.len(), 1);
        assert!(schema.constraints[0].name.contains("uq"));

        // Add a PRIMARY KEY constraint
        let add_pk_plan = QueryPlan {
            root: PlanNode::AlterTable(AlterTableNode {
                table_name: "constraint_test".to_string(),
                operations: vec![
                    PlanAlterOperation::AddConstraint(CreateTableConstraint::PrimaryKey {
                        columns: vec!["id".to_string()],
                    }),
                ],
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&add_pk_plan).unwrap();

        // Verify the primary key was set
        let schema = executor.context.read().unwrap().get_table_schema("constraint_test").unwrap().clone();
        assert_eq!(schema.primary_key, Some(vec!["id".to_string()]));
        assert_eq!(schema.constraints.len(), 2);

        // Try adding another PRIMARY KEY (should fail)
        let add_dup_pk_plan = QueryPlan {
            root: PlanNode::AlterTable(AlterTableNode {
                table_name: "constraint_test".to_string(),
                operations: vec![
                    PlanAlterOperation::AddConstraint(CreateTableConstraint::PrimaryKey {
                        columns: vec!["email".to_string()],
                    }),
                ],
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        let result = executor.execute(&add_dup_pk_plan);
        assert!(result.is_err());

        // Drop the UNIQUE constraint by name
        let pk_constraint_name = {
            let schema = executor.context.read().unwrap().get_table_schema("constraint_test").unwrap().clone();
            schema.constraints.iter()
                .find(|c| matches!(c.constraint_type, StoredConstraintType::Unique { .. }))
                .map(|c| c.name.clone())
                .unwrap()
        };

        let drop_constraint_plan = QueryPlan {
            root: PlanNode::AlterTable(AlterTableNode {
                table_name: "constraint_test".to_string(),
                operations: vec![
                    PlanAlterOperation::DropConstraint {
                        name: pk_constraint_name,
                    },
                ],
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&drop_constraint_plan).unwrap();

        // Verify the constraint was dropped
        let schema = executor.context.read().unwrap().get_table_schema("constraint_test").unwrap().clone();
        assert_eq!(schema.constraints.len(), 1);
        // Only the PK constraint should remain
        assert!(matches!(schema.constraints[0].constraint_type, StoredConstraintType::PrimaryKey { .. }));
    }

    #[test]
    fn test_create_table_with_constraints() {
        use crate::planner::{CreateTableNode, CreateColumnDef, CreateTableConstraint};

        let executor = Executor::new(ExecutionContext::new());

        // Create a table with constraints
        let create_plan = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "users".to_string(),
                columns: vec![
                    CreateColumnDef {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        primary_key: true,
                        unique: false,
                    },
                    CreateColumnDef {
                        name: "email".to_string(),
                        data_type: DataType::Text,
                        nullable: false,
                        default: None,
                        primary_key: false,
                        unique: true, // Column-level unique
                    },
                    CreateColumnDef {
                        name: "name".to_string(),
                        data_type: DataType::Text,
                        nullable: true,
                        default: None,
                        primary_key: false,
                        unique: false,
                    },
                ],
                constraints: vec![
                    CreateTableConstraint::Unique {
                        columns: vec!["name".to_string()],
                    },
                ],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_plan).unwrap();

        // Verify schema has primary key and constraints
        let schema = executor.context.read().unwrap().get_table_schema("users").unwrap().clone();
        assert_eq!(schema.primary_key, Some(vec!["id".to_string()]));
        // Should have: pk, uq for name, uq for email (column-level)
        assert!(schema.constraints.len() >= 2);
    }

    #[test]
    fn test_column_default_values() {
        use crate::planner::{CreateTableNode, CreateColumnDef, InsertNode, InsertPlanSource, PlanExpression, PlanLiteral};

        let executor = Executor::new(ExecutionContext::new());

        // Create a table with a default value
        let create_plan = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "defaults_test".to_string(),
                columns: vec![
                    CreateColumnDef {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        primary_key: true,
                        unique: false,
                    },
                    CreateColumnDef {
                        name: "status".to_string(),
                        data_type: DataType::Text,
                        nullable: false,
                        default: Some(PlanExpression::Literal(PlanLiteral::String("active".to_string()))),
                        primary_key: false,
                        unique: false,
                    },
                    CreateColumnDef {
                        name: "count".to_string(),
                        data_type: DataType::Integer,
                        nullable: true,
                        default: Some(PlanExpression::Literal(PlanLiteral::Integer(0))),
                        primary_key: false,
                        unique: false,
                    },
                ],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_plan).unwrap();

        // Verify default values are stored in schema
        let schema = executor.context.read().unwrap().get_table_schema("defaults_test").unwrap().clone();
        assert_eq!(schema.columns[1].default, Some(Value::String("active".to_string())));
        assert_eq!(schema.columns[2].default, Some(Value::Integer(0)));

        // Insert a row specifying only the id column
        let insert_plan = QueryPlan {
            root: PlanNode::Insert(InsertNode {
                table_name: "defaults_test".to_string(),
                columns: vec!["id".to_string()],
                source: InsertPlanSource::Values(vec![
                    vec![PlanExpression::Literal(PlanLiteral::Integer(1))],
                ]),
            }),
            estimated_cost: 1.0,
            estimated_rows: 1,
        };
        executor.execute(&insert_plan).unwrap();

        // Verify the default values were used
        let context = executor.context.read().unwrap();
        let table = context.get_table("defaults_test").unwrap();
        let table_data = table.read().unwrap();

        assert_eq!(table_data.rows.len(), 1);
        assert_eq!(table_data.rows[0].values[0], Value::Integer(1)); // id
        assert_eq!(table_data.rows[0].values[1], Value::String("active".to_string())); // status default
        assert_eq!(table_data.rows[0].values[2], Value::Integer(0)); // count default
    }

    #[test]
    fn test_alter_column_default() {
        use crate::planner::{CreateTableNode, CreateColumnDef, AlterTableNode, PlanAlterOperation, PlanExpression, PlanLiteral};

        let executor = Executor::new(ExecutionContext::new());

        // Create a table
        let create_plan = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "alter_default_test".to_string(),
                columns: vec![
                    CreateColumnDef {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        primary_key: true,
                        unique: false,
                    },
                    CreateColumnDef {
                        name: "value".to_string(),
                        data_type: DataType::Integer,
                        nullable: true,
                        default: None,
                        primary_key: false,
                        unique: false,
                    },
                ],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_plan).unwrap();

        // Alter column to add a default
        let alter_plan = QueryPlan {
            root: PlanNode::AlterTable(AlterTableNode {
                table_name: "alter_default_test".to_string(),
                operations: vec![
                    PlanAlterOperation::AlterColumn {
                        name: "value".to_string(),
                        data_type: None,
                        set_not_null: None,
                        set_default: Some(Some(PlanExpression::Literal(PlanLiteral::Integer(42)))),
                    },
                ],
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&alter_plan).unwrap();

        // Verify the default was set
        let schema = executor.context.read().unwrap().get_table_schema("alter_default_test").unwrap().clone();
        assert_eq!(schema.columns[1].default, Some(Value::Integer(42)));

        // Alter column to drop the default
        let drop_default_plan = QueryPlan {
            root: PlanNode::AlterTable(AlterTableNode {
                table_name: "alter_default_test".to_string(),
                operations: vec![
                    PlanAlterOperation::AlterColumn {
                        name: "value".to_string(),
                        data_type: None,
                        set_not_null: None,
                        set_default: Some(None), // Drop default
                    },
                ],
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&drop_default_plan).unwrap();

        // Verify the default was removed
        let schema = executor.context.read().unwrap().get_table_schema("alter_default_test").unwrap().clone();
        assert_eq!(schema.columns[1].default, None);
    }

    #[test]
    fn test_parameterized_query() {
        use crate::planner::{CreateTableNode, CreateColumnDef, InsertNode, InsertPlanSource, FilterNode};

        let executor = Executor::new(ExecutionContext::new());

        // Create a test table
        let create_plan = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "params_test".to_string(),
                columns: vec![
                    CreateColumnDef {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        primary_key: true,
                        unique: false,
                    },
                    CreateColumnDef {
                        name: "name".to_string(),
                        data_type: DataType::Text,
                        nullable: true,
                        default: None,
                        primary_key: false,
                        unique: false,
                    },
                ],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_plan).unwrap();

        // Insert some data
        let insert_plan = QueryPlan {
            root: PlanNode::Insert(InsertNode {
                table_name: "params_test".to_string(),
                columns: vec!["id".to_string(), "name".to_string()],
                source: InsertPlanSource::Values(vec![
                    vec![
                        PlanExpression::Literal(PlanLiteral::Integer(1)),
                        PlanExpression::Literal(PlanLiteral::String("Alice".to_string())),
                    ],
                    vec![
                        PlanExpression::Literal(PlanLiteral::Integer(2)),
                        PlanExpression::Literal(PlanLiteral::String("Bob".to_string())),
                    ],
                    vec![
                        PlanExpression::Literal(PlanLiteral::Integer(3)),
                        PlanExpression::Literal(PlanLiteral::String("Charlie".to_string())),
                    ],
                ]),
            }),
            estimated_cost: 1.0,
            estimated_rows: 3,
        };
        executor.execute(&insert_plan).unwrap();

        // Create a query with a placeholder: SELECT * FROM params_test WHERE id = $1
        let query_plan = QueryPlan {
            root: PlanNode::Filter(FilterNode {
                input: Box::new(PlanNode::Scan(ScanNode {
                    table_name: "params_test".to_string(),
                    alias: None,
                    columns: vec!["id".to_string(), "name".to_string()],
                    index_scan: None,
                })),
                predicate: PlanExpression::BinaryOp {
                    left: Box::new(PlanExpression::Column {
                        table: None,
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                    }),
                    op: PlanBinaryOp::Equal,
                    right: Box::new(PlanExpression::Placeholder(1)),
                },
            }),
            estimated_cost: 10.0,
            estimated_rows: 1,
        };

        // Execute with parameter $1 = 2 (should find Bob)
        let result = executor.execute_with_params(&query_plan, &[Value::Integer(2)]).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[0], Value::Integer(2));
        assert_eq!(result.rows[0].values[1], Value::String("Bob".to_string()));

        // Execute with parameter $1 = 1 (should find Alice)
        let result = executor.execute_with_params(&query_plan, &[Value::Integer(1)]).unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values[1], Value::String("Alice".to_string()));

        // Execute with parameter $1 = 99 (should find nothing)
        let result = executor.execute_with_params(&query_plan, &[Value::Integer(99)]).unwrap();
        assert_eq!(result.rows.len(), 0);
    }

    #[test]
    fn test_parameterized_insert() {
        use crate::planner::{CreateTableNode, CreateColumnDef, InsertNode, InsertPlanSource};

        let executor = Executor::new(ExecutionContext::new());

        // Create a test table
        let create_plan = QueryPlan {
            root: PlanNode::CreateTable(CreateTableNode {
                table_name: "param_insert_test".to_string(),
                columns: vec![
                    CreateColumnDef {
                        name: "id".to_string(),
                        data_type: DataType::Integer,
                        nullable: false,
                        default: None,
                        primary_key: true,
                        unique: false,
                    },
                    CreateColumnDef {
                        name: "value".to_string(),
                        data_type: DataType::Text,
                        nullable: true,
                        default: None,
                        primary_key: false,
                        unique: false,
                    },
                ],
                constraints: vec![],
                if_not_exists: false,
            }),
            estimated_cost: 1.0,
            estimated_rows: 0,
        };
        executor.execute(&create_plan).unwrap();

        // Create an INSERT with placeholders: INSERT INTO param_insert_test (id, value) VALUES ($1, $2)
        let insert_plan = QueryPlan {
            root: PlanNode::Insert(InsertNode {
                table_name: "param_insert_test".to_string(),
                columns: vec!["id".to_string(), "value".to_string()],
                source: InsertPlanSource::Values(vec![
                    vec![
                        PlanExpression::Placeholder(1),
                        PlanExpression::Placeholder(2),
                    ],
                ]),
            }),
            estimated_cost: 1.0,
            estimated_rows: 1,
        };

        // Execute insert with parameters
        executor.execute_with_params(&insert_plan, &[
            Value::Integer(42),
            Value::String("test value".to_string()),
        ]).unwrap();

        // Verify the data was inserted
        let context = executor.context.read().unwrap();
        let table = context.get_table("param_insert_test").unwrap();
        let table_data = table.read().unwrap();

        assert_eq!(table_data.rows.len(), 1);
        assert_eq!(table_data.rows[0].values[0], Value::Integer(42));
        assert_eq!(table_data.rows[0].values[1], Value::String("test value".to_string()));
    }
}
