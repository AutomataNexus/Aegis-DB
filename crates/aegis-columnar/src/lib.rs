//! Aegis Columnar — columnar / OLAP engine for the Aegis database.
//!
//! Named tables with a fixed typed schema, stored **column-major** (one vector
//! per column), supporting predicate scans and group-by aggregation
//! (`count` / `sum` / `min` / `max` / `avg`) — the analytical counterpart to the
//! row-oriented SQL engine.

pub mod engine;
pub mod table;
pub mod types;

pub use engine::{ColumnarEngine, EngineSnapshot, TableStats};
pub use table::Table;
pub use types::{
    AggFunc, AggSpec, ColumnDef, ColumnType, ColumnarError, CompareOp, Condition, GroupRow, Value,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn cols() -> Vec<ColumnDef> {
        vec![
            ColumnDef {
                name: "region".into(),
                ty: ColumnType::Text,
            },
            ColumnDef {
                name: "product".into(),
                ty: ColumnType::Text,
            },
            ColumnDef {
                name: "amount".into(),
                ty: ColumnType::Float,
            },
            ColumnDef {
                name: "qty".into(),
                ty: ColumnType::Int,
            },
        ]
    }

    fn seeded() -> ColumnarEngine {
        let e = ColumnarEngine::new();
        e.create_table("sales", cols()).unwrap();
        let rows = [
            ("east", "widget", 100.0, 2),
            ("east", "gadget", 50.0, 1),
            ("west", "widget", 200.0, 4),
            ("west", "widget", 25.0, 1),
            ("east", "widget", 75.0, 3),
        ];
        let rows: Vec<serde_json::Value> = rows
            .iter()
            .map(|(r, p, a, q)| serde_json::json!({"region": r, "product": p, "amount": a, "qty": q}))
            .collect();
        assert_eq!(e.insert_many("sales", &rows).unwrap(), 5);
        e
    }

    fn cond(column: &str, op: CompareOp, value: serde_json::Value) -> Condition {
        Condition {
            column: column.into(),
            op,
            value,
        }
    }
    fn agg(func: AggFunc, column: &str) -> AggSpec {
        AggSpec {
            func,
            column: column.into(),
        }
    }

    #[test]
    fn global_aggregate() {
        let e = seeded();
        let rows = e
            .aggregate(
                "sales",
                &[],
                &[
                    agg(AggFunc::Count, "*"),
                    agg(AggFunc::Sum, "amount"),
                    agg(AggFunc::Avg, "qty"),
                ],
                &[],
            )
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].values[0], Value::Int(5));
        assert_eq!(rows[0].values[1], Value::Float(450.0));
        assert_eq!(rows[0].values[2], Value::Float(11.0 / 5.0));
    }

    #[test]
    fn group_by_with_filter() {
        let e = seeded();
        // sum(amount) per region, only widget rows.
        let mut rows = e
            .aggregate(
                "sales",
                &["region".into()],
                &[agg(AggFunc::Sum, "amount"), agg(AggFunc::Count, "*")],
                &[cond("product", CompareOp::Eq, serde_json::json!("widget"))],
            )
            .unwrap();
        rows.sort_by(|a, b| a.keys[0].group_key().cmp(&b.keys[0].group_key()));
        assert_eq!(rows.len(), 2);
        // east widgets: 100 + 75 = 175 (2 rows)
        assert_eq!(rows[0].keys[0], Value::Text("east".into()));
        assert_eq!(rows[0].values[0], Value::Float(175.0));
        assert_eq!(rows[0].values[1], Value::Int(2));
        // west widgets: 200 + 25 = 225 (2 rows)
        assert_eq!(rows[1].keys[0], Value::Text("west".into()));
        assert_eq!(rows[1].values[0], Value::Float(225.0));
    }

    #[test]
    fn min_max_with_range_filter() {
        let e = seeded();
        let rows = e
            .aggregate(
                "sales",
                &[],
                &[agg(AggFunc::Min, "amount"), agg(AggFunc::Max, "amount")],
                &[cond("amount", CompareOp::Gte, serde_json::json!(50))],
            )
            .unwrap();
        // amounts >= 50: 100, 50, 200, 75
        assert_eq!(rows[0].values[0], Value::Float(50.0));
        assert_eq!(rows[0].values[1], Value::Float(200.0));
    }

    #[test]
    fn scan_projection_and_limit() {
        let e = seeded();
        let out = e
            .scan(
                "sales",
                &["region".into(), "amount".into()],
                &[cond("region", CompareOp::Eq, serde_json::json!("west"))],
                Some(1),
            )
            .unwrap();
        assert_eq!(out.len(), 1);
        let obj = out[0].as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert_eq!(obj["region"], serde_json::json!("west"));
        assert!(obj.contains_key("amount"));
    }

    #[test]
    fn distinct_values() {
        let e = seeded();
        let regions = e.distinct("sales", "region").unwrap();
        assert_eq!(regions.len(), 2);
        let products = e.distinct("sales", "product").unwrap();
        assert_eq!(products.len(), 2);
    }

    #[test]
    fn errors_and_validation() {
        let e = ColumnarEngine::new();
        assert!(matches!(
            e.create_table("t", vec![]),
            Err(ColumnarError::EmptySchema)
        ));
        e.create_table("t", cols()).unwrap();
        // unknown column on insert
        assert!(matches!(
            e.insert("t", serde_json::json!({"nope": 1}).as_object().unwrap()),
            Err(ColumnarError::UnknownColumn(_))
        ));
        // type mismatch
        assert!(matches!(
            e.insert("t", serde_json::json!({"amount": "x"}).as_object().unwrap()),
            Err(ColumnarError::TypeMismatch)
        ));
        // missing table
        assert!(matches!(
            e.aggregate("nope", &[], &[agg(AggFunc::Count, "*")], &[]),
            Err(ColumnarError::TableNotFound(_))
        ));
    }

    #[test]
    fn snapshot_roundtrip() {
        let e = seeded();
        let bytes = serde_json::to_vec(&e.snapshot()).unwrap();
        let restored = ColumnarEngine::new();
        restored.load_snapshot(serde_json::from_slice(&bytes).unwrap());
        assert_eq!(restored.table_stats("sales").unwrap().rows, 5);
        let rows = restored
            .aggregate("sales", &[], &[agg(AggFunc::Sum, "amount")], &[])
            .unwrap();
        assert_eq!(rows[0].values[0], Value::Float(450.0));
    }
}
