//! Aegis Ledger — ledger / append-only engine for the Aegis database.
//!
//! Named ledgers of immutable, **hash-chained** entries: each entry's hash
//! covers the previous entry's hash, so the log is tamper-evident — any
//! retroactive edit breaks every link after it and is caught by verification.
//! Append-only (no update/delete of entries), with integrity verification and
//! snapshot persistence.

pub mod engine;
pub mod types;

pub use engine::{EngineSnapshot, LedgerEngine, LedgerStats};
pub use types::{chain_hash, LedgerEntry, LedgerError, VerifyResult, GENESIS_HASH};

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> LedgerEngine {
        let e = LedgerEngine::new();
        e.create_ledger("audit").unwrap();
        e.append(
            "audit",
            serde_json::json!({"event": "create", "user": "alice"}),
            Some(1),
        )
        .unwrap();
        e.append(
            "audit",
            serde_json::json!({"event": "update", "user": "bob"}),
            Some(2),
        )
        .unwrap();
        e.append(
            "audit",
            serde_json::json!({"event": "delete", "user": "alice"}),
            Some(3),
        )
        .unwrap();
        e
    }

    #[test]
    fn append_assigns_seq_and_chains() {
        let e = seeded();
        let e0 = e.get("audit", 0).unwrap().unwrap();
        let e1 = e.get("audit", 1).unwrap().unwrap();
        let e2 = e.get("audit", 2).unwrap().unwrap();
        assert_eq!((e0.seq, e1.seq, e2.seq), (0, 1, 2));
        // Genesis link, then each prev_hash equals the prior entry's hash.
        assert_eq!(e0.prev_hash, GENESIS_HASH);
        assert_eq!(e1.prev_hash, e0.hash);
        assert_eq!(e2.prev_hash, e1.hash);
        // Hashes are 128-bit (32 hex chars) and distinct.
        assert_eq!(e0.hash.len(), 32);
        assert_ne!(e0.hash, e1.hash);
    }

    #[test]
    fn verify_passes_for_intact_chain() {
        let e = seeded();
        let v = e.verify("audit").unwrap();
        assert!(v.valid);
        assert_eq!(v.entries, 3);
        assert!(v.broken_at.is_none());
    }

    #[test]
    fn tampering_breaks_verification() {
        let e = seeded();
        // Serialize the snapshot to JSON, retroactively edit entry 1's payload
        // (without recomputing its hash), reload, and re-verify.
        let mut json: serde_json::Value = serde_json::to_value(e.snapshot()).unwrap();
        json["ledgers"]["audit"]["entries"][1]["payload"]["user"] = serde_json::json!("mallory");
        let doctored: EngineSnapshot = serde_json::from_value(json).unwrap();
        let tampered = LedgerEngine::new();
        tampered.load_snapshot(doctored);

        let v = tampered.verify("audit").unwrap();
        assert!(!v.valid);
        assert_eq!(v.broken_at, Some(1));
    }

    #[test]
    fn range_and_stats() {
        let e = seeded();
        let page = e.range("audit", 1, Some(10)).unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].seq, 1);

        let stats = e.ledger_stats("audit").unwrap();
        assert_eq!(stats.entries, 3);
        assert_eq!(stats.tip_hash, e.get("audit", 2).unwrap().unwrap().hash);
    }

    #[test]
    fn errors() {
        let e = LedgerEngine::new();
        e.create_ledger("l").unwrap();
        assert!(matches!(
            e.create_ledger("l"),
            Err(LedgerError::LedgerExists(_))
        ));
        assert!(matches!(
            e.append("nope", serde_json::json!({}), None),
            Err(LedgerError::LedgerNotFound(_))
        ));
        assert!(e.get("l", 99).unwrap().is_none());
    }

    #[test]
    fn snapshot_roundtrip_preserves_chain() {
        let e = seeded();
        let bytes = serde_json::to_vec(&e.snapshot()).unwrap();
        let restored = LedgerEngine::new();
        restored.load_snapshot(serde_json::from_slice(&bytes).unwrap());
        assert_eq!(restored.ledger_stats("audit").unwrap().entries, 3);
        assert!(restored.verify("audit").unwrap().valid);
        // Appending after restore continues the same chain.
        let e3 = restored
            .append("audit", serde_json::json!({"event": "read"}), Some(4))
            .unwrap();
        assert_eq!(e3.seq, 3);
        assert!(restored.verify("audit").unwrap().valid);
    }

    // ---- Empty / isolation / lifecycle -------------------------------------

    #[test]
    fn empty_ledger_verifies_with_genesis_tip() {
        let e = LedgerEngine::new();
        e.create_ledger("l").unwrap();
        let v = e.verify("l").unwrap();
        assert!(v.valid);
        assert_eq!(v.entries, 0);
        assert_eq!(e.ledger_stats("l").unwrap().tip_hash, GENESIS_HASH);
    }

    #[test]
    fn ledgers_are_independent() {
        let e = LedgerEngine::new();
        e.create_ledger("a").unwrap();
        e.create_ledger("b").unwrap();
        let a0 = e
            .append("a", serde_json::json!({"x": 1}), Some(10))
            .unwrap();
        let b0 = e
            .append("b", serde_json::json!({"x": 1}), Some(10))
            .unwrap();
        // Same payload + timestamp + seq + genesis => identical hash across ledgers.
        assert_eq!(a0.hash, b0.hash);
        // Diverge them; each still verifies on its own.
        e.append("a", serde_json::json!({"x": 2}), Some(11))
            .unwrap();
        assert!(e.verify("a").unwrap().valid);
        assert!(e.verify("b").unwrap().valid);
        assert_eq!(e.ledger_stats("a").unwrap().entries, 2);
        assert_eq!(e.ledger_stats("b").unwrap().entries, 1);
    }

    #[test]
    fn lifecycle_and_errors() {
        let e = LedgerEngine::new();
        e.create_ledger("a").unwrap();
        e.create_ledger("b").unwrap();
        assert_eq!(e.list_ledgers(), vec!["a", "b"]);
        assert!(matches!(
            e.create_ledger("a"),
            Err(LedgerError::LedgerExists(_))
        ));
        e.drop_ledger("a").unwrap();
        assert!(matches!(
            e.drop_ledger("a"),
            Err(LedgerError::LedgerNotFound(_))
        ));
        assert!(matches!(
            e.append("a", serde_json::json!({}), None),
            Err(LedgerError::LedgerNotFound(_))
        ));
        assert!(matches!(e.verify("a"), Err(LedgerError::LedgerNotFound(_))));
        assert!(matches!(e.get("a", 0), Err(LedgerError::LedgerNotFound(_))));
    }

    // ---- Reads --------------------------------------------------------------

    #[test]
    fn range_offset_limit_and_out_of_bounds() {
        let e = seeded();
        assert_eq!(e.range("audit", 0, None).unwrap().len(), 3);
        let page = e.range("audit", 1, Some(1)).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].seq, 1);
        assert!(e.range("audit", 99, None).unwrap().is_empty()); // past the end
        assert!(e.get("audit", 99).unwrap().is_none());
    }

    #[test]
    fn explicit_vs_auto_timestamps() {
        let e = LedgerEngine::new();
        e.create_ledger("l").unwrap();
        let a = e.append("l", serde_json::json!({}), Some(42)).unwrap();
        assert_eq!(a.timestamp, 42);
        let b = e.append("l", serde_json::json!({}), None).unwrap();
        let c = e.append("l", serde_json::json!({}), None).unwrap();
        assert!(c.timestamp > b.timestamp); // auto clock advances
    }

    #[test]
    fn append_is_deterministic() {
        // Same ledger contents (payloads + timestamps) => identical hashes.
        let build = || {
            let e = LedgerEngine::new();
            e.create_ledger("l").unwrap();
            e.append("l", serde_json::json!({"a": 1, "b": [2, 3]}), Some(1))
                .unwrap();
            e.append("l", serde_json::json!({"z": "end"}), Some(2))
                .unwrap();
            e
        };
        let h1 = build().ledger_stats("l").unwrap().tip_hash;
        let h2 = build().ledger_stats("l").unwrap().tip_hash;
        assert_eq!(h1, h2);
    }

    // ---- Tamper detection across every chained field ------------------------

    /// Doctor one entry field in a serialized snapshot, reload, and return the
    /// `broken_at` reported by verify (None if it still verifies).
    fn tamper_at(field: &str, idx: usize, value: serde_json::Value) -> Option<u64> {
        let e = seeded();
        let mut json: serde_json::Value = serde_json::to_value(e.snapshot()).unwrap();
        json["ledgers"]["audit"]["entries"][idx][field] = value;
        let doctored: EngineSnapshot = serde_json::from_value(json).unwrap();
        let t = LedgerEngine::new();
        t.load_snapshot(doctored);
        t.verify("audit").unwrap().broken_at
    }

    #[test]
    fn tampering_any_field_breaks_chain() {
        // Editing the payload of entry 1 breaks at 1.
        assert_eq!(
            tamper_at("payload", 1, serde_json::json!({"evil": true})),
            Some(1)
        );
        // Editing the stored timestamp breaks (it's covered by the hash).
        assert_eq!(tamper_at("timestamp", 0, serde_json::json!(9999)), Some(0));
        // Corrupting the stored hash itself breaks at that entry.
        assert_eq!(
            tamper_at(
                "hash",
                2,
                serde_json::json!("deadbeefdeadbeefdeadbeefdeadbeef")
            ),
            Some(2)
        );
        // Rewriting a prev_hash to break the link is caught.
        assert_eq!(
            tamper_at(
                "prev_hash",
                2,
                serde_json::json!("00000000000000000000000000000000")
            ),
            Some(2)
        );
        // A truthful intact chain still verifies (control).
        assert_eq!(seeded().verify("audit").unwrap().broken_at, None);
    }
}
