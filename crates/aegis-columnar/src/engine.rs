//! The columnar / OLAP engine: named column-major tables with predicate scans
//! and group-by aggregation, plus snapshot persistence.

use crate::table::Table;
use crate::types::{AggSpec, ColumnDef, ColumnType, ColumnarError, Condition, GroupRow};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Infer a table schema from a JSON row, used when a table is auto-created on
/// first insert. Booleans → `bool`, integers → `int`, other numbers → `float`,
/// everything else (strings, and null/array/object as a fallback) → `text`.
fn infer_schema(row: &serde_json::Map<String, serde_json::Value>) -> Vec<ColumnDef> {
    row.iter()
        .map(|(name, value)| {
            let ty = match value {
                serde_json::Value::Bool(_) => ColumnType::Bool,
                serde_json::Value::Number(n) if n.is_i64() || n.is_u64() => ColumnType::Int,
                serde_json::Value::Number(_) => ColumnType::Float,
                _ => ColumnType::Text,
            };
            ColumnDef {
                name: name.clone(),
                ty,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct TableStats {
    pub name: String,
    pub rows: usize,
    pub columns: Vec<ColumnDef>,
}

/// Multi-table columnar engine.
pub struct ColumnarEngine {
    tables: RwLock<HashMap<String, Table>>,
}

impl Default for ColumnarEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ColumnarEngine {
    pub fn new() -> Self {
        Self {
            tables: RwLock::new(HashMap::new()),
        }
    }

    pub fn create_table(
        &self,
        name: impl Into<String>,
        schema: Vec<ColumnDef>,
    ) -> Result<(), ColumnarError> {
        let name = name.into();
        let table = Table::new(schema)?;
        let mut tables = self.tables.write();
        if tables.contains_key(&name) {
            return Err(ColumnarError::TableExists(name));
        }
        tables.insert(name, table);
        Ok(())
    }

    pub fn drop_table(&self, name: &str) -> Result<(), ColumnarError> {
        self.tables
            .write()
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| ColumnarError::TableNotFound(name.to_string()))
    }

    pub fn list_tables(&self) -> Vec<String> {
        let mut v: Vec<String> = self.tables.read().keys().cloned().collect();
        v.sort();
        v
    }

    pub fn table_stats(&self, name: &str) -> Option<TableStats> {
        let tables = self.tables.read();
        let t = tables.get(name)?;
        Some(TableStats {
            name: name.to_string(),
            rows: t.row_count(),
            columns: t.schema().to_vec(),
        })
    }

    /// Append a single row (JSON object) to a table. If the table does not yet
    /// exist it is created on demand, with its schema inferred from the row's
    /// JSON value types (`int` / `float` / `bool` / `text`).
    pub fn insert(
        &self,
        table: &str,
        row: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), ColumnarError> {
        let mut tables = self.tables.write();
        if !tables.contains_key(table) {
            let schema = infer_schema(row);
            if schema.is_empty() {
                return Err(ColumnarError::EmptySchema);
            }
            tables.insert(table.to_string(), Table::new(schema)?);
        }
        let t = tables
            .get_mut(table)
            .expect("table present after auto-create");
        t.insert_row(row)
    }

    /// Append many rows; returns the number inserted. Stops at the first bad
    /// row (rows before it are kept). A missing table is created on demand with
    /// its schema inferred from the first row.
    pub fn insert_many(
        &self,
        table: &str,
        rows: &[serde_json::Value],
    ) -> Result<usize, ColumnarError> {
        let mut tables = self.tables.write();
        if !tables.contains_key(table) {
            let first = rows.first().and_then(|r| r.as_object());
            match first {
                Some(obj) => {
                    let schema = infer_schema(obj);
                    if schema.is_empty() {
                        return Err(ColumnarError::EmptySchema);
                    }
                    tables.insert(table.to_string(), Table::new(schema)?);
                }
                None => return Err(ColumnarError::TableNotFound(table.to_string())),
            }
        }
        let t = tables
            .get_mut(table)
            .expect("table present after auto-create");
        let mut n = 0;
        for row in rows {
            let obj = row.as_object().ok_or(ColumnarError::TypeMismatch)?;
            t.insert_row(obj)?;
            n += 1;
        }
        Ok(n)
    }

    /// Project columns (empty = all) with a filter and optional row limit.
    pub fn scan(
        &self,
        table: &str,
        columns: &[String],
        filter: &[Condition],
        limit: Option<usize>,
    ) -> Result<Vec<serde_json::Value>, ColumnarError> {
        let tables = self.tables.read();
        let t = tables
            .get(table)
            .ok_or_else(|| ColumnarError::TableNotFound(table.to_string()))?;
        t.scan(columns, filter, limit)
    }

    /// Group-by aggregation over the rows matching `filter`.
    pub fn aggregate(
        &self,
        table: &str,
        group_by: &[String],
        aggs: &[AggSpec],
        filter: &[Condition],
    ) -> Result<Vec<GroupRow>, ColumnarError> {
        let tables = self.tables.read();
        let t = tables
            .get(table)
            .ok_or_else(|| ColumnarError::TableNotFound(table.to_string()))?;
        t.aggregate(group_by, aggs, filter)
    }

    /// Distinct non-null values of a column.
    pub fn distinct(
        &self,
        table: &str,
        column: &str,
    ) -> Result<Vec<crate::types::Value>, ColumnarError> {
        let tables = self.tables.read();
        let t = tables
            .get(table)
            .ok_or_else(|| ColumnarError::TableNotFound(table.to_string()))?;
        t.distinct(column)
    }

    // ---- Persistence --------------------------------------------------------

    pub fn snapshot(&self) -> EngineSnapshot {
        EngineSnapshot {
            tables: self.tables.read().clone(),
        }
    }

    pub fn load_snapshot(&self, snap: EngineSnapshot) {
        *self.tables.write() = snap.tables;
    }
}

#[derive(Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub tables: HashMap<String, Table>,
}
