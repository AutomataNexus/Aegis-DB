//! The wide-column engine: named tables of row-keyed rows, each row a sparse,
//! dynamic set of columns. Cells carry a write timestamp and conflicting writes
//! resolve last-write-wins. Rows are stored sorted by key for range scans.

use crate::types::{Cell, RowResult, WideColumnError};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize)]
pub struct TableStats {
    pub name: String,
    pub rows: usize,
    pub cells: usize,
}

/// A row is a sorted map of column name → cell.
type Row = BTreeMap<String, Cell>;

/// A table: row key → row, kept sorted by row key.
#[derive(Default, Clone, Serialize, Deserialize)]
struct Table {
    rows: BTreeMap<String, Row>,
}

/// Multi-table wide-column store.
pub struct WideColumnEngine {
    tables: RwLock<HashMap<String, Table>>,
    /// Monotonic logical clock for writes that don't supply a timestamp.
    clock: AtomicU64,
}

impl Default for WideColumnEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl WideColumnEngine {
    pub fn new() -> Self {
        Self {
            tables: RwLock::new(HashMap::new()),
            clock: AtomicU64::new(1),
        }
    }

    fn next_ts(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::SeqCst)
    }

    pub fn create_table(&self, name: impl Into<String>) -> Result<(), WideColumnError> {
        let name = name.into();
        let mut tables = self.tables.write();
        if tables.contains_key(&name) {
            return Err(WideColumnError::TableExists(name));
        }
        tables.insert(name, Table::default());
        Ok(())
    }

    pub fn drop_table(&self, name: &str) -> Result<(), WideColumnError> {
        self.tables
            .write()
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| WideColumnError::TableNotFound(name.to_string()))
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
            rows: t.rows.len(),
            cells: t.rows.values().map(|r| r.len()).sum(),
        })
    }

    /// Set one or more columns on a row. Each column's cell is stamped with
    /// `timestamp` (or the next logical clock tick if `None`). Existing cells are
    /// only overwritten when the new timestamp is **greater** (last-write-wins;
    /// equal timestamps keep the existing value).
    pub fn put(
        &self,
        table: &str,
        row_key: impl Into<String>,
        columns: serde_json::Map<String, serde_json::Value>,
        timestamp: Option<u64>,
    ) -> Result<(), WideColumnError> {
        if columns.is_empty() {
            return Err(WideColumnError::EmptyWrite);
        }
        let ts = timestamp.unwrap_or_else(|| self.next_ts());
        let mut tables = self.tables.write();
        let t = tables
            .get_mut(table)
            .ok_or_else(|| WideColumnError::TableNotFound(table.to_string()))?;
        let row = t.rows.entry(row_key.into()).or_default();
        for (col, value) in columns {
            match row.get(&col) {
                Some(existing) if existing.timestamp >= ts => {} // older/equal write loses
                _ => {
                    row.insert(
                        col,
                        Cell {
                            value,
                            timestamp: ts,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn project(row: &Row, columns: &[String]) -> RowResult {
        let mut cols = serde_json::Map::new();
        let mut tss = serde_json::Map::new();
        let take = |c: &str,
                    cell: &Cell,
                    cols: &mut serde_json::Map<_, _>,
                    tss: &mut serde_json::Map<_, _>| {
            cols.insert(c.to_string(), cell.value.clone());
            tss.insert(c.to_string(), serde_json::json!(cell.timestamp));
        };
        if columns.is_empty() {
            for (c, cell) in row {
                take(c, cell, &mut cols, &mut tss);
            }
        } else {
            for c in columns {
                if let Some(cell) = row.get(c) {
                    take(c, cell, &mut cols, &mut tss);
                }
            }
        }
        RowResult {
            key: String::new(),
            columns: cols,
            timestamps: tss,
        }
    }

    /// Get a row's columns (empty `columns` = all), or `None` if the row is
    /// absent. Projected columns that don't exist on the row are omitted.
    pub fn get(
        &self,
        table: &str,
        row_key: &str,
        columns: &[String],
    ) -> Result<Option<RowResult>, WideColumnError> {
        let tables = self.tables.read();
        let t = tables
            .get(table)
            .ok_or_else(|| WideColumnError::TableNotFound(table.to_string()))?;
        Ok(t.rows.get(row_key).map(|row| {
            let mut r = Self::project(row, columns);
            r.key = row_key.to_string();
            r
        }))
    }

    /// Get a single cell's value, or `None` if the row/column is absent.
    pub fn get_cell(
        &self,
        table: &str,
        row_key: &str,
        column: &str,
    ) -> Result<Option<serde_json::Value>, WideColumnError> {
        let tables = self.tables.read();
        let t = tables
            .get(table)
            .ok_or_else(|| WideColumnError::TableNotFound(table.to_string()))?;
        Ok(t.rows
            .get(row_key)
            .and_then(|row| row.get(column))
            .map(|cell| cell.value.clone()))
    }

    /// Delete a single column from a row; returns whether it existed.
    pub fn delete_cell(
        &self,
        table: &str,
        row_key: &str,
        column: &str,
    ) -> Result<bool, WideColumnError> {
        let mut tables = self.tables.write();
        let t = tables
            .get_mut(table)
            .ok_or_else(|| WideColumnError::TableNotFound(table.to_string()))?;
        let removed = t
            .rows
            .get_mut(row_key)
            .map(|row| row.remove(column).is_some())
            .unwrap_or(false);
        // Drop the row entirely once its last column is gone.
        if let Some(row) = t.rows.get(row_key) {
            if row.is_empty() {
                t.rows.remove(row_key);
            }
        }
        Ok(removed)
    }

    /// Delete an entire row; returns whether it existed.
    pub fn delete_row(&self, table: &str, row_key: &str) -> Result<bool, WideColumnError> {
        let mut tables = self.tables.write();
        let t = tables
            .get_mut(table)
            .ok_or_else(|| WideColumnError::TableNotFound(table.to_string()))?;
        Ok(t.rows.remove(row_key).is_some())
    }

    /// Scan rows in key order. `start` is inclusive, `end` exclusive; `prefix`
    /// filters to keys with that prefix; `limit` caps the count. `columns`
    /// projects each row (empty = all).
    #[allow(clippy::too_many_arguments)]
    pub fn scan(
        &self,
        table: &str,
        start: Option<&str>,
        end: Option<&str>,
        prefix: Option<&str>,
        columns: &[String],
        limit: Option<usize>,
    ) -> Result<Vec<RowResult>, WideColumnError> {
        let tables = self.tables.read();
        let t = tables
            .get(table)
            .ok_or_else(|| WideColumnError::TableNotFound(table.to_string()))?;
        let lo = start.unwrap_or("");
        let cap = limit.unwrap_or(usize::MAX);
        let out = t
            .rows
            .range(lo.to_string()..)
            .take_while(|(k, _)| match end {
                Some(e) => k.as_str() < e,
                None => true,
            })
            .filter(|(k, _)| prefix.map(|p| k.starts_with(p)).unwrap_or(true))
            .take(cap)
            .map(|(k, row)| {
                let mut r = Self::project(row, columns);
                r.key = k.clone();
                r
            })
            .collect();
        Ok(out)
    }

    // ---- Persistence --------------------------------------------------------

    pub fn snapshot(&self) -> EngineSnapshot {
        EngineSnapshot {
            tables: self.tables.read().clone(),
            clock: self.clock.load(Ordering::SeqCst),
        }
    }

    pub fn load_snapshot(&self, snap: EngineSnapshot) {
        *self.tables.write() = snap.tables;
        self.clock.store(snap.clock.max(1), Ordering::SeqCst);
    }
}

#[derive(Serialize, Deserialize)]
pub struct EngineSnapshot {
    tables: HashMap<String, Table>,
    #[serde(default)]
    clock: u64,
}
