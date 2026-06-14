//! Aegis Wide-Column — wide-column engine for the Aegis database.
//!
//! Cassandra / Bigtable-style tables: rows keyed by a row key, each row a
//! **sparse, dynamic** set of columns. Every cell carries a write timestamp and
//! conflicting writes resolve **last-write-wins**. Rows are kept sorted by key,
//! so range / prefix scans are ordered. Snapshot persisted.

pub mod engine;
pub mod types;

pub use engine::{EngineSnapshot, TableStats, WideColumnEngine};
pub use types::{Cell, RowResult, WideColumnError};

#[cfg(test)]
mod tests {
    use super::*;

    fn cols(pairs: &[(&str, serde_json::Value)]) -> serde_json::Map<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn seeded() -> WideColumnEngine {
        let e = WideColumnEngine::new();
        e.create_table("users").unwrap();
        e.put(
            "users",
            "user:1",
            cols(&[
                ("name", serde_json::json!("Alice")),
                ("age", serde_json::json!(30)),
            ]),
            None,
        )
        .unwrap();
        e.put(
            "users",
            "user:2",
            cols(&[("name", serde_json::json!("Bob"))]),
            None,
        )
        .unwrap();
        e.put(
            "users",
            "user:3",
            cols(&[
                ("name", serde_json::json!("Carol")),
                ("city", serde_json::json!("NYC")),
            ]),
            None,
        )
        .unwrap();
        e
    }

    #[test]
    fn sparse_dynamic_columns() {
        let e = seeded();
        // user:1 has name+age, user:2 only name, user:3 name+city — all in one table.
        let r1 = e.get("users", "user:1", &[]).unwrap().unwrap();
        assert_eq!(r1.columns.len(), 2);
        let r2 = e.get("users", "user:2", &[]).unwrap().unwrap();
        assert_eq!(r2.columns.len(), 1);
        let r3 = e.get("users", "user:3", &[]).unwrap().unwrap();
        assert!(r3.columns.contains_key("city"));
        assert!(!r1.columns.contains_key("city"));
    }

    #[test]
    fn partial_update_merges_columns() {
        let e = seeded();
        // Add a column to an existing row without disturbing the others.
        e.put(
            "users",
            "user:2",
            cols(&[("age", serde_json::json!(25))]),
            None,
        )
        .unwrap();
        let r = e.get("users", "user:2", &[]).unwrap().unwrap();
        assert_eq!(r.columns["name"], serde_json::json!("Bob"));
        assert_eq!(r.columns["age"], serde_json::json!(25));
    }

    #[test]
    fn last_write_wins_by_timestamp() {
        let e = WideColumnEngine::new();
        e.create_table("t").unwrap();
        e.put(
            "t",
            "r",
            cols(&[("v", serde_json::json!("first"))]),
            Some(100),
        )
        .unwrap();
        // An older write loses.
        e.put(
            "t",
            "r",
            cols(&[("v", serde_json::json!("stale"))]),
            Some(50),
        )
        .unwrap();
        assert_eq!(
            e.get_cell("t", "r", "v").unwrap().unwrap(),
            serde_json::json!("first")
        );
        // A newer write wins.
        e.put(
            "t",
            "r",
            cols(&[("v", serde_json::json!("latest"))]),
            Some(200),
        )
        .unwrap();
        assert_eq!(
            e.get_cell("t", "r", "v").unwrap().unwrap(),
            serde_json::json!("latest")
        );
    }

    #[test]
    fn projection_and_get_cell() {
        let e = seeded();
        let r = e
            .get("users", "user:1", &["name".to_string()])
            .unwrap()
            .unwrap();
        assert_eq!(r.columns.len(), 1);
        assert_eq!(r.columns["name"], serde_json::json!("Alice"));
        assert_eq!(
            e.get_cell("users", "user:1", "age").unwrap().unwrap(),
            serde_json::json!(30)
        );
        assert!(e.get_cell("users", "user:1", "nope").unwrap().is_none());
    }

    #[test]
    fn range_and_prefix_scan_ordered() {
        let e = seeded();
        // Full scan is ordered by row key.
        let all = e.scan("users", None, None, None, &[], None).unwrap();
        let keys: Vec<&str> = all.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec!["user:1", "user:2", "user:3"]);

        // Range [user:2, user:3) -> just user:2.
        let ranged = e
            .scan("users", Some("user:2"), Some("user:3"), None, &[], None)
            .unwrap();
        assert_eq!(ranged.len(), 1);
        assert_eq!(ranged[0].key, "user:2");

        // Prefix + limit.
        let limited = e
            .scan("users", None, None, Some("user:"), &[], Some(2))
            .unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn delete_cell_and_row() {
        let e = seeded();
        assert!(e.delete_cell("users", "user:1", "age").unwrap());
        assert!(e.get_cell("users", "user:1", "age").unwrap().is_none());
        // Deleting the last column drops the row.
        assert!(e.delete_cell("users", "user:2", "name").unwrap());
        assert!(e.get("users", "user:2", &[]).unwrap().is_none());

        assert!(e.delete_row("users", "user:3").unwrap());
        assert!(!e.delete_row("users", "user:3").unwrap());
        assert_eq!(e.table_stats("users").unwrap().rows, 1);
    }

    #[test]
    fn errors() {
        let e = WideColumnEngine::new();
        e.create_table("t").unwrap();
        assert!(matches!(
            e.create_table("t"),
            Err(WideColumnError::TableExists(_))
        ));
        assert!(matches!(
            e.put("t", "r", serde_json::Map::new(), None),
            Err(WideColumnError::EmptyWrite)
        ));
        assert!(matches!(
            e.get("nope", "r", &[]),
            Err(WideColumnError::TableNotFound(_))
        ));
    }

    #[test]
    fn snapshot_roundtrip() {
        let e = seeded();
        let bytes = serde_json::to_vec(&e.snapshot()).unwrap();
        let restored = WideColumnEngine::new();
        restored.load_snapshot(serde_json::from_slice(&bytes).unwrap());
        assert_eq!(restored.table_stats("users").unwrap().rows, 3);
        let r = restored.get("users", "user:1", &[]).unwrap().unwrap();
        assert_eq!(r.columns["name"], serde_json::json!("Alice"));
    }
}
