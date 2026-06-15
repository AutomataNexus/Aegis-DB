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

    #[test]
    fn insert_auto_creates_table_with_inferred_schema() {
        let e = ColumnarEngine::new();
        // No create_table — the schema is inferred from the first rows.
        let n = e
            .insert_many(
                "auto",
                &[
                    serde_json::json!({"region":"east","amount":10.5,"qty":2,"active":true}),
                    serde_json::json!({"region":"west","amount":20.0,"qty":3,"active":false}),
                ],
            )
            .unwrap();
        assert_eq!(n, 2);
        let stats = e.table_stats("auto").unwrap();
        assert_eq!(stats.rows, 2);
        // Inferred types: text / float / int / bool.
        let ty = |name: &str| stats.columns.iter().find(|c| c.name == name).unwrap().ty;
        assert_eq!(ty("region"), ColumnType::Text);
        assert_eq!(ty("amount"), ColumnType::Float);
        assert_eq!(ty("qty"), ColumnType::Int);
        assert_eq!(ty("active"), ColumnType::Bool);
        // The inferred table is immediately queryable.
        let g = e
            .aggregate("auto", &[], &[agg(AggFunc::Sum, "amount")], &[])
            .unwrap();
        assert_eq!(g[0].values[0], Value::Float(30.5));
        // Single-row insert also auto-creates.
        e.insert("solo", serde_json::json!({"x": 1}).as_object().unwrap())
            .unwrap();
        assert_eq!(e.table_stats("solo").unwrap().rows, 1);
    }

    // ---- Comparison operators ----------------------------------------------

    #[test]
    fn every_comparison_operator() {
        let e = seeded();
        let count_where = |c: Condition| {
            e.aggregate("sales", &[], &[agg(AggFunc::Count, "*")], &[c])
                .unwrap()[0]
                .values[0]
                .clone()
        };
        // amounts: 100, 50, 200, 25, 75
        assert_eq!(
            count_where(cond("amount", CompareOp::Eq, serde_json::json!(50))),
            Value::Int(1)
        );
        assert_eq!(
            count_where(cond("amount", CompareOp::Ne, serde_json::json!(50))),
            Value::Int(4)
        );
        assert_eq!(
            count_where(cond("amount", CompareOp::Lt, serde_json::json!(75))),
            Value::Int(2)
        ); // 50,25
        assert_eq!(
            count_where(cond("amount", CompareOp::Lte, serde_json::json!(75))),
            Value::Int(3)
        ); // 50,25,75
        assert_eq!(
            count_where(cond("amount", CompareOp::Gt, serde_json::json!(75))),
            Value::Int(2)
        ); // 100,200
        assert_eq!(
            count_where(cond("amount", CompareOp::Gte, serde_json::json!(75))),
            Value::Int(3)
        ); // 100,200,75
    }

    #[test]
    fn text_comparison_is_lexical() {
        let e = seeded();
        // region < "f" => "east" rows only (4 of 5; "west" excluded).
        let rows = e
            .scan(
                "sales",
                &["region".into()],
                &[cond("region", CompareOp::Lt, serde_json::json!("f"))],
                None,
            )
            .unwrap();
        assert!(rows
            .iter()
            .all(|r| r["region"] == serde_json::json!("east")));
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn conditions_are_anded() {
        let e = seeded();
        let rows = e
            .aggregate(
                "sales",
                &[],
                &[agg(AggFunc::Count, "*")],
                &[
                    cond("region", CompareOp::Eq, serde_json::json!("east")),
                    cond("amount", CompareOp::Gt, serde_json::json!(60)),
                ],
            )
            .unwrap();
        // east rows with amount>60: 100, 75 => 2
        assert_eq!(rows[0].values[0], Value::Int(2));
    }

    // ---- Multi-column group-by ---------------------------------------------

    #[test]
    fn group_by_two_columns() {
        let e = seeded();
        let mut rows = e
            .aggregate(
                "sales",
                &["region".into(), "product".into()],
                &[agg(AggFunc::Count, "*"), agg(AggFunc::Sum, "amount")],
                &[],
            )
            .unwrap();
        rows.sort_by(|a, b| {
            (a.keys[0].group_key(), a.keys[1].group_key())
                .cmp(&(b.keys[0].group_key(), b.keys[1].group_key()))
        });
        // (east,gadget)=50, (east,widget)=175/2, (west,widget)=225/2
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0].keys,
            vec![Value::Text("east".into()), Value::Text("gadget".into())]
        );
        assert_eq!(rows[0].values[0], Value::Int(1));
        assert_eq!(rows[1].values[1], Value::Float(175.0));
        assert_eq!(rows[2].values[1], Value::Float(225.0));
    }

    // ---- Nulls & empties ----------------------------------------------------

    #[test]
    fn nulls_distinguish_count_star_from_count_col() {
        let e = ColumnarEngine::new();
        e.create_table("t", cols()).unwrap();
        e.insert_many(
            "t",
            &[
                serde_json::json!({"region": "x", "amount": 10}),
                serde_json::json!({"region": "x"}), // amount missing => null
                serde_json::json!({"region": "x", "amount": null}), // explicit null
            ],
        )
        .unwrap();
        let rows = e
            .aggregate(
                "t",
                &[],
                &[
                    agg(AggFunc::Count, "*"),
                    agg(AggFunc::Count, "amount"),
                    agg(AggFunc::Sum, "amount"),
                    agg(AggFunc::Avg, "amount"),
                ],
                &[],
            )
            .unwrap();
        assert_eq!(rows[0].values[0], Value::Int(3)); // count(*) = all rows
        assert_eq!(rows[0].values[1], Value::Int(1)); // count(amount) = non-null
        assert_eq!(rows[0].values[2], Value::Float(10.0));
        assert_eq!(rows[0].values[3], Value::Float(10.0)); // avg ignores nulls
    }

    #[test]
    fn aggregate_over_no_matching_rows() {
        let e = seeded();
        let rows = e
            .aggregate(
                "sales",
                &[],
                &[
                    agg(AggFunc::Count, "*"),
                    agg(AggFunc::Sum, "amount"),
                    agg(AggFunc::Min, "amount"),
                    agg(AggFunc::Avg, "amount"),
                ],
                &[cond("region", CompareOp::Eq, serde_json::json!("nowhere"))],
            )
            .unwrap();
        // Global aggregate over an empty set: one group, zeros / nulls.
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].values[0], Value::Int(0));
        assert_eq!(rows[0].values[1], Value::Float(0.0));
        assert_eq!(rows[0].values[2], Value::Null); // min of nothing
        assert_eq!(rows[0].values[3], Value::Null); // avg of nothing
    }

    #[test]
    fn sum_of_text_column_is_zero() {
        // Non-numeric cells are ignored by numeric aggregates.
        let e = seeded();
        let rows = e
            .aggregate(
                "sales",
                &[],
                &[agg(AggFunc::Sum, "region"), agg(AggFunc::Min, "region")],
                &[],
            )
            .unwrap();
        assert_eq!(rows[0].values[0], Value::Float(0.0));
        assert_eq!(rows[0].values[1], Value::Null);
    }

    // ---- Scan, distinct, schema --------------------------------------------

    #[test]
    fn scan_all_columns_and_over_limit() {
        let e = seeded();
        let all = e.scan("sales", &[], &[], None).unwrap();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0].as_object().unwrap().len(), 4); // every column projected
        let capped = e.scan("sales", &[], &[], Some(100)).unwrap();
        assert_eq!(capped.len(), 5); // limit beyond row count is fine
    }

    #[test]
    fn distinct_handles_nulls_and_unknown_column() {
        let e = ColumnarEngine::new();
        e.create_table("t", cols()).unwrap();
        e.insert_many(
            "t",
            &[
                serde_json::json!({"region": "a"}),
                serde_json::json!({"region": "b"}),
                serde_json::json!({"region": "a"}),
                serde_json::json!({}), // null region
            ],
        )
        .unwrap();
        let d = e.distinct("t", "region").unwrap();
        assert_eq!(d, vec![Value::Text("a".into()), Value::Text("b".into())]); // nulls excluded, sorted
        assert!(matches!(
            e.distinct("t", "nope"),
            Err(ColumnarError::UnknownColumn(_))
        ));
    }

    #[test]
    fn int_column_coerces_float_and_truncates() {
        let e = ColumnarEngine::new();
        e.create_table("t", cols()).unwrap();
        // qty is Int; a float value is truncated rather than rejected.
        e.insert("t", serde_json::json!({"qty": 3.9}).as_object().unwrap())
            .unwrap();
        let rows = e.scan("t", &["qty".into()], &[], None).unwrap();
        assert_eq!(rows[0]["qty"], serde_json::json!(3));
    }

    #[test]
    fn schema_rejects_duplicate_and_empty() {
        let e = ColumnarEngine::new();
        assert!(matches!(
            e.create_table("a", vec![]),
            Err(ColumnarError::EmptySchema)
        ));
        let dup = vec![
            ColumnDef {
                name: "x".into(),
                ty: ColumnType::Int,
            },
            ColumnDef {
                name: "x".into(),
                ty: ColumnType::Text,
            },
        ];
        assert!(matches!(
            e.create_table("b", dup),
            Err(ColumnarError::DuplicateColumn(_))
        ));
    }

    #[test]
    fn unknown_aggregate_or_filter_column_errors() {
        let e = seeded();
        assert!(matches!(
            e.aggregate("sales", &[], &[agg(AggFunc::Sum, "nope")], &[]),
            Err(ColumnarError::UnknownColumn(_))
        ));
        assert!(matches!(
            e.aggregate("sales", &["nope".into()], &[agg(AggFunc::Count, "*")], &[]),
            Err(ColumnarError::UnknownColumn(_))
        ));
        assert!(matches!(
            e.scan(
                "sales",
                &[],
                &[cond("nope", CompareOp::Eq, serde_json::json!(1))],
                None
            ),
            Err(ColumnarError::UnknownColumn(_))
        ));
    }

    #[test]
    fn table_lifecycle() {
        let e = ColumnarEngine::new();
        e.create_table("a", cols()).unwrap();
        assert!(matches!(
            e.create_table("a", cols()),
            Err(ColumnarError::TableExists(_))
        ));
        e.create_table("b", cols()).unwrap();
        assert_eq!(e.list_tables(), vec!["a", "b"]);
        e.drop_table("a").unwrap();
        assert!(matches!(
            e.drop_table("a"),
            Err(ColumnarError::TableNotFound(_))
        ));
        assert!(e.table_stats("a").is_none());
    }
}
