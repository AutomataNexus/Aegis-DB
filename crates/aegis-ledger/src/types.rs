//! Core types for the ledger / append-only engine.

use serde::{Deserialize, Serialize};

/// The `prev_hash` of the first entry in every ledger.
pub const GENESIS_HASH: &str = "00000000000000000000000000000000";

/// One immutable ledger entry. Each entry's `hash` covers the previous entry's
/// hash, so the entries form a tamper-evident chain: editing any past entry
/// changes its hash and breaks every link after it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub seq: u64,
    pub timestamp: u64,
    pub payload: serde_json::Value,
    pub prev_hash: String,
    pub hash: String,
}

/// The result of verifying a ledger's hash chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub valid: bool,
    pub entries: usize,
    /// The sequence number of the first entry whose hash/link is invalid, if any.
    pub broken_at: Option<u64>,
}

/// Errors returned by the ledger engine.
#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("ledger '{0}' not found")]
    LedgerNotFound(String),
    #[error("ledger '{0}' already exists")]
    LedgerExists(String),
    #[error("entry {0} not found")]
    EntryNotFound(u64),
}

/// The chain hash for an entry: a 128-bit FNV-1a digest (two lanes, 32 hex
/// chars) over `prev_hash || seq || timestamp || canonical(payload)`. Because
/// each entry mixes in the previous hash, any retroactive edit is detectable by
/// re-verification. This is a fast non-cryptographic digest — it detects
/// corruption and accidental/naive tampering, not a forging adversary.
pub fn chain_hash(
    prev_hash: &str,
    seq: u64,
    timestamp: u64,
    payload: &serde_json::Value,
) -> String {
    // serde_json serializes object keys in sorted (BTreeMap) order by default,
    // so the canonical form is stable for the same logical value.
    let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();

    let feed = |h: &mut u64, prime: u64| {
        let mix = |bytes: &[u8], h: &mut u64| {
            for &b in bytes {
                *h ^= b as u64;
                *h = h.wrapping_mul(prime);
            }
        };
        mix(prev_hash.as_bytes(), h);
        mix(&seq.to_le_bytes(), h);
        mix(&timestamp.to_le_bytes(), h);
        mix(&payload_bytes, h);
    };

    // Two independent lanes (different offset bases, same prime) → 128 bits.
    let mut h1: u64 = 0xcbf2_9ce4_8422_2325;
    let mut h2: u64 = 0x84222325_cbf29ce4;
    feed(&mut h1, 0x0000_0100_0000_01b3);
    feed(&mut h2, 0x0000_0100_0000_01b3);
    format!("{h1:016x}{h2:016x}")
}
