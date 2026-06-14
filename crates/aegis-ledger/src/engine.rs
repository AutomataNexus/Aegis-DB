//! The ledger / append-only engine: named ledgers of immutable, hash-chained
//! entries, with integrity verification and snapshot persistence.

use crate::types::{chain_hash, LedgerEntry, LedgerError, VerifyResult, GENESIS_HASH};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Serialize)]
pub struct LedgerStats {
    pub name: String,
    pub entries: usize,
    /// The hash of the most recent entry (the chain tip), or genesis if empty.
    pub tip_hash: String,
}

/// A single append-only ledger. Entries are stored in sequence order; `seq`
/// equals the index.
#[derive(Default, Clone, Serialize, Deserialize)]
struct Ledger {
    entries: Vec<LedgerEntry>,
}

impl Ledger {
    fn tip(&self) -> &str {
        self.entries
            .last()
            .map(|e| e.hash.as_str())
            .unwrap_or(GENESIS_HASH)
    }
}

/// Multi-ledger append-only store.
pub struct LedgerEngine {
    ledgers: RwLock<HashMap<String, Ledger>>,
    /// Monotonic logical clock for appends that don't supply a timestamp.
    clock: AtomicU64,
}

impl Default for LedgerEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LedgerEngine {
    pub fn new() -> Self {
        Self {
            ledgers: RwLock::new(HashMap::new()),
            clock: AtomicU64::new(1),
        }
    }

    fn next_ts(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::SeqCst)
    }

    pub fn create_ledger(&self, name: impl Into<String>) -> Result<(), LedgerError> {
        let name = name.into();
        let mut ledgers = self.ledgers.write();
        if ledgers.contains_key(&name) {
            return Err(LedgerError::LedgerExists(name));
        }
        ledgers.insert(name, Ledger::default());
        Ok(())
    }

    pub fn drop_ledger(&self, name: &str) -> Result<(), LedgerError> {
        self.ledgers
            .write()
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| LedgerError::LedgerNotFound(name.to_string()))
    }

    pub fn list_ledgers(&self) -> Vec<String> {
        let mut v: Vec<String> = self.ledgers.read().keys().cloned().collect();
        v.sort();
        v
    }

    pub fn ledger_stats(&self, name: &str) -> Option<LedgerStats> {
        let ledgers = self.ledgers.read();
        let l = ledgers.get(name)?;
        Some(LedgerStats {
            name: name.to_string(),
            entries: l.entries.len(),
            tip_hash: l.tip().to_string(),
        })
    }

    /// Append a payload to the ledger, returning the new (immutable) entry. The
    /// entry's hash chains off the current tip.
    pub fn append(
        &self,
        ledger: &str,
        payload: serde_json::Value,
        timestamp: Option<u64>,
    ) -> Result<LedgerEntry, LedgerError> {
        let ts = timestamp.unwrap_or_else(|| self.next_ts());
        let mut ledgers = self.ledgers.write();
        let l = ledgers
            .get_mut(ledger)
            .ok_or_else(|| LedgerError::LedgerNotFound(ledger.to_string()))?;
        let seq = l.entries.len() as u64;
        let prev_hash = l.tip().to_string();
        let hash = chain_hash(&prev_hash, seq, ts, &payload);
        let entry = LedgerEntry {
            seq,
            timestamp: ts,
            payload,
            prev_hash,
            hash,
        };
        l.entries.push(entry.clone());
        Ok(entry)
    }

    /// Fetch an entry by sequence number.
    pub fn get(&self, ledger: &str, seq: u64) -> Result<Option<LedgerEntry>, LedgerError> {
        let ledgers = self.ledgers.read();
        let l = ledgers
            .get(ledger)
            .ok_or_else(|| LedgerError::LedgerNotFound(ledger.to_string()))?;
        Ok(l.entries.get(seq as usize).cloned())
    }

    /// Read entries in sequence order starting at `start`, capped at `limit`.
    pub fn range(
        &self,
        ledger: &str,
        start: u64,
        limit: Option<usize>,
    ) -> Result<Vec<LedgerEntry>, LedgerError> {
        let ledgers = self.ledgers.read();
        let l = ledgers
            .get(ledger)
            .ok_or_else(|| LedgerError::LedgerNotFound(ledger.to_string()))?;
        let cap = limit.unwrap_or(usize::MAX);
        Ok(l.entries
            .iter()
            .skip(start as usize)
            .take(cap)
            .cloned()
            .collect())
    }

    /// Verify the ledger's hash chain end to end: every entry's hash must equal
    /// the recomputed hash, and its `prev_hash` must equal the prior entry's
    /// hash (genesis for the first). Returns the first broken sequence, if any.
    pub fn verify(&self, ledger: &str) -> Result<VerifyResult, LedgerError> {
        let ledgers = self.ledgers.read();
        let l = ledgers
            .get(ledger)
            .ok_or_else(|| LedgerError::LedgerNotFound(ledger.to_string()))?;
        let mut prev = GENESIS_HASH.to_string();
        for (i, e) in l.entries.iter().enumerate() {
            let expected = chain_hash(&e.prev_hash, e.seq, e.timestamp, &e.payload);
            let ok = e.seq == i as u64 && e.prev_hash == prev && e.hash == expected;
            if !ok {
                return Ok(VerifyResult {
                    valid: false,
                    entries: l.entries.len(),
                    broken_at: Some(i as u64),
                });
            }
            prev = e.hash.clone();
        }
        Ok(VerifyResult {
            valid: true,
            entries: l.entries.len(),
            broken_at: None,
        })
    }

    // ---- Persistence --------------------------------------------------------

    pub fn snapshot(&self) -> EngineSnapshot {
        EngineSnapshot {
            ledgers: self.ledgers.read().clone(),
            clock: self.clock.load(Ordering::SeqCst),
        }
    }

    pub fn load_snapshot(&self, snap: EngineSnapshot) {
        *self.ledgers.write() = snap.ledgers;
        self.clock.store(snap.clock.max(1), Ordering::SeqCst);
    }
}

#[derive(Serialize, Deserialize)]
pub struct EngineSnapshot {
    ledgers: HashMap<String, Ledger>,
    #[serde(default)]
    clock: u64,
}
