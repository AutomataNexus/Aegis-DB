//! A single columnar table: schema + column-major value storage, with predicate
//! scans and group-by aggregation.

use crate::types::{
    AggFunc, AggSpec, ColumnDef, ColumnarError, CompareOp, Condition, GroupRow, Value,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

/// A columnar table. Values are stored **column-major** — one `Vec<Value>` per
/// column — so aggregations touch only the columns they reference.
#[derive(Clone, Serialize, Deserialize)]
pub struct Table {
    schema: Vec<ColumnDef>,
    name_idx: HashMap<String, usize>,
    columns: Vec<Vec<Value>>,
    rows: usize,
}

impl Table {
    pub fn new(schema: Vec<ColumnDef>) -> Result<Self, ColumnarError> {
        if schema.is_empty() {
            return Err(ColumnarError::EmptySchema);
        }
        let mut name_idx = HashMap::new();
        for (i, c) in schema.iter().enumerate() {
            if name_idx.insert(c.name.clone(), i).is_some() {
                return Err(ColumnarError::DuplicateColumn(c.name.clone()));
            }
        }
        let columns = vec![Vec::new(); schema.len()];
        Ok(Self {
            schema,
            name_idx,
            columns,
            rows: 0,
        })
    }

    pub fn row_count(&self) -> usize {
        self.rows
    }

    pub fn schema(&self) -> &[ColumnDef] {
        &self.schema
    }

    fn col_index(&self, name: &str) -> Result<usize, ColumnarError> {
        self.name_idx
            .get(name)
            .copied()
            .ok_or_else(|| ColumnarError::UnknownColumn(name.to_string()))
    }

    /// Append one row from a JSON object. Missing columns become `Null`;
    /// unknown keys are rejected.
    pub fn insert_row(
        &mut self,
        row: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), ColumnarError> {
        for k in row.keys() {
            if !self.name_idx.contains_key(k) {
                return Err(ColumnarError::UnknownColumn(k.clone()));
            }
        }
        // Coerce everything first so a bad value leaves the table unchanged.
        let mut coerced: Vec<Value> = Vec::with_capacity(self.schema.len());
        for def in &self.schema {
            let v = match row.get(&def.name) {
                Some(j) => Value::coerce(j, def.ty)?,
                None => Value::Null,
            };
            coerced.push(v);
        }
        for (col, v) in self.columns.iter_mut().zip(coerced) {
            col.push(v);
        }
        self.rows += 1;
        Ok(())
    }

    /// Row indices (in insertion order) matching every condition (ANDed).
    fn matching_rows(&self, filter: &[Condition]) -> Result<Vec<usize>, ColumnarError> {
        // Pre-resolve each condition's column index + typed comparison value.
        let mut resolved: Vec<(usize, CompareOp, Value)> = Vec::with_capacity(filter.len());
        for c in filter {
            let idx = self.col_index(&c.column)?;
            let val = Value::coerce(&c.value, self.schema[idx].ty)?;
            resolved.push((idx, c.op, val));
        }
        let mut out = Vec::new();
        for r in 0..self.rows {
            if resolved
                .iter()
                .all(|(idx, op, val)| compare(&self.columns[*idx][r], *op, val))
            {
                out.push(r);
            }
        }
        Ok(out)
    }

    /// Project selected columns (or all, if `columns` is empty) for the rows
    /// matching `filter`, as JSON objects. `limit` caps the row count.
    pub fn scan(
        &self,
        columns: &[String],
        filter: &[Condition],
        limit: Option<usize>,
    ) -> Result<Vec<serde_json::Value>, ColumnarError> {
        let proj: Vec<usize> = if columns.is_empty() {
            (0..self.schema.len()).collect()
        } else {
            columns
                .iter()
                .map(|c| self.col_index(c))
                .collect::<Result<_, _>>()?
        };
        let rows = self.matching_rows(filter)?;
        let cap = limit.unwrap_or(rows.len()).min(rows.len());
        let mut out = Vec::with_capacity(cap);
        for &r in rows.iter().take(cap) {
            let mut obj = serde_json::Map::new();
            for &ci in &proj {
                obj.insert(
                    self.schema[ci].name.clone(),
                    value_to_json(&self.columns[ci][r]),
                );
            }
            out.push(serde_json::Value::Object(obj));
        }
        Ok(out)
    }

    /// Group the rows matching `filter` by `group_by` (zero or more columns) and
    /// compute each aggregate. With no group-by columns, returns a single global
    /// group.
    pub fn aggregate(
        &self,
        group_by: &[String],
        aggs: &[AggSpec],
        filter: &[Condition],
    ) -> Result<Vec<GroupRow>, ColumnarError> {
        let gcols: Vec<usize> = group_by
            .iter()
            .map(|c| self.col_index(c))
            .collect::<Result<_, _>>()?;
        // Validate aggregate columns up front ("*" allowed only for count).
        for a in aggs {
            if a.column == "*" {
                continue;
            }
            self.col_index(&a.column)?;
        }
        let rows = self.matching_rows(filter)?;

        // group key string -> (key values, accumulators)
        let mut groups: HashMap<String, (Vec<Value>, Vec<Acc>)> = HashMap::new();
        let mut order: Vec<String> = Vec::new();

        // A global aggregate (no group-by) always returns exactly one row, even
        // over an empty set — SQL semantics, where `count(*)` is then 0. Seed the
        // single global group so it survives a zero-row result.
        if gcols.is_empty() {
            order.push(String::new());
            groups.insert(
                String::new(),
                (Vec::new(), aggs.iter().map(|_| Acc::default()).collect()),
            );
        }
        for &r in &rows {
            let key_vals: Vec<Value> = gcols
                .iter()
                .map(|&ci| self.columns[ci][r].clone())
                .collect();
            let gkey = key_vals
                .iter()
                .map(|v| v.group_key())
                .collect::<Vec<_>>()
                .join("\u{1}");
            let entry = groups.entry(gkey.clone()).or_insert_with(|| {
                order.push(gkey.clone());
                (
                    key_vals.clone(),
                    aggs.iter().map(|_| Acc::default()).collect(),
                )
            });
            for (i, a) in aggs.iter().enumerate() {
                let cell = if a.column == "*" {
                    None
                } else {
                    Some(&self.columns[self.name_idx[&a.column]][r])
                };
                entry.1[i].update(a.func, cell);
            }
        }

        let mut out = Vec::with_capacity(order.len());
        for gkey in order {
            let (keys, accs) = groups.remove(&gkey).expect("group key present");
            let values = accs
                .iter()
                .zip(aggs)
                .map(|(acc, a)| acc.finish(a.func))
                .collect();
            out.push(GroupRow { keys, values });
        }
        Ok(out)
    }

    /// Distinct non-null values in a column (sorted by their group key).
    pub fn distinct(&self, column: &str) -> Result<Vec<Value>, ColumnarError> {
        let idx = self.col_index(column)?;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for v in &self.columns[idx] {
            if v.is_null() {
                continue;
            }
            if seen.insert(v.group_key()) {
                out.push(v.clone());
            }
        }
        out.sort_by_key(|v| v.group_key());
        Ok(out)
    }
}

/// Accumulator for one aggregate over one group.
#[derive(Default, Clone)]
struct Acc {
    count: i64,
    sum: f64,
    min: Option<f64>,
    max: Option<f64>,
}

impl Acc {
    fn update(&mut self, func: AggFunc, cell: Option<&Value>) {
        match func {
            // count(*) counts rows; count(col) counts non-null cells.
            AggFunc::Count => match cell {
                None => self.count += 1,
                Some(v) if !v.is_null() => self.count += 1,
                _ => {}
            },
            _ => {
                if let Some(n) = cell.and_then(|v| v.as_f64()) {
                    self.count += 1;
                    self.sum += n;
                    self.min = Some(self.min.map_or(n, |m| m.min(n)));
                    self.max = Some(self.max.map_or(n, |m| m.max(n)));
                }
            }
        }
    }

    fn finish(&self, func: AggFunc) -> Value {
        match func {
            AggFunc::Count => Value::Int(self.count),
            AggFunc::Sum => Value::Float(self.sum),
            AggFunc::Min => self.min.map(Value::Float).unwrap_or(Value::Null),
            AggFunc::Max => self.max.map(Value::Float).unwrap_or(Value::Null),
            AggFunc::Avg => {
                if self.count > 0 {
                    Value::Float(self.sum / self.count as f64)
                } else {
                    Value::Null
                }
            }
        }
    }
}

fn compare(cell: &Value, op: CompareOp, rhs: &Value) -> bool {
    use std::cmp::Ordering;
    // Ordering: numerics compared numerically; text lexically; null only equal
    // to null. Mixed/!comparable → not-equal semantics.
    let ord = match (cell, rhs) {
        (Value::Null, Value::Null) => Some(Ordering::Equal),
        (Value::Null, _) | (_, Value::Null) => None,
        (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
        _ => match (cell.as_f64(), rhs.as_f64()) {
            (Some(a), Some(b)) => a.partial_cmp(&b),
            _ => None,
        },
    };
    match op {
        CompareOp::Eq => ord == Some(Ordering::Equal),
        CompareOp::Ne => ord != Some(Ordering::Equal),
        CompareOp::Lt => ord == Some(Ordering::Less),
        CompareOp::Lte => matches!(ord, Some(Ordering::Less | Ordering::Equal)),
        CompareOp::Gt => ord == Some(Ordering::Greater),
        CompareOp::Gte => matches!(ord, Some(Ordering::Greater | Ordering::Equal)),
    }
}

/// Convert a typed cell back to JSON.
pub fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Int(i) => serde_json::json!(i),
        Value::Float(f) => serde_json::json!(f),
        Value::Bool(b) => serde_json::json!(b),
        Value::Text(s) => serde_json::json!(s),
        Value::Null => serde_json::Value::Null,
    }
}
