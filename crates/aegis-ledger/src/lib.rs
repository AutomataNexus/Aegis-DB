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
}
