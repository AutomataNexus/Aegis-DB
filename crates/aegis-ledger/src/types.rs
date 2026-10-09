//! Core types for the ledger / append-only engine.

use serde::{Deserialize, Serialize};

/// The `prev_hash` of the first entry in every SHA-256 (version 2) ledger.
pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// The `prev_hash` of the first entry in every legacy FNV-1a (version 1) ledger.
pub const GENESIS_HASH_V1: &str = "00000000000000000000000000000000";

/// Chain-hash algorithm of ledgers written before v0.6: 128-bit FNV-1a.
pub const HASH_VERSION_FNV1A: u8 = 1;

/// Chain-hash algorithm of ledgers created from v0.6 on: SHA-256.
pub const HASH_VERSION_SHA256: u8 = 2;

/// The genesis `prev_hash` for a ledger of the given hash version.
pub fn genesis_hash(version: u8) -> &'static str {
    if version == HASH_VERSION_FNV1A {
        GENESIS_HASH_V1
    } else {
        GENESIS_HASH
    }
}

/// The chain hash for an entry under the given hash version.
pub fn chain_hash_versioned(
    version: u8,
    prev_hash: &str,
    seq: u64,
    timestamp: u64,
    payload: &serde_json::Value,
) -> String {
    if version == HASH_VERSION_FNV1A {
        chain_hash_fnv1a(prev_hash, seq, timestamp, payload)
    } else {
        chain_hash(prev_hash, seq, timestamp, payload)
    }
}

/// The legacy version 1 chain hash: a 128-bit FNV-1a digest (two lanes, 32 hex
/// chars) over `prev_hash || seq || timestamp || canonical(payload)`. Kept so
/// ledgers written before v0.6 still verify and keep chaining.
pub fn chain_hash_fnv1a(
    prev_hash: &str,
    seq: u64,
    timestamp: u64,
    payload: &serde_json::Value,
) -> String {
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
    let mut h1: u64 = 0xcbf2_9ce4_8422_2325;
    let mut h2: u64 = 0x84222325_cbf29ce4;
    feed(&mut h1, 0x0000_0100_0000_01b3);
    feed(&mut h2, 0x0000_0100_0000_01b3);
    format!("{h1:016x}{h2:016x}")
}

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

/// The chain hash for an entry: a SHA-256 digest (64 hex chars) over
/// `prev_hash || seq || timestamp || canonical(payload)`. Each entry mixes in
/// the previous hash, so the entries form a tamper-evident chain — a
/// collision/preimage-resistant hash means a retroactive edit cannot be
/// disguised by forging a colliding payload, and breaks every link after it.
pub fn chain_hash(
    prev_hash: &str,
    seq: u64,
    timestamp: u64,
    payload: &serde_json::Value,
) -> String {
    use sha2::{Digest, Sha256};
    let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();

    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(seq.to_le_bytes());
    hasher.update(timestamp.to_le_bytes());
    hasher.update(&payload_bytes);

    let mut out = String::with_capacity(64);
    for b in hasher.finalize() {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}
