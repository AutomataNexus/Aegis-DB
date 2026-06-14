//! Core types for the object / blob store.

use serde::{Deserialize, Serialize};

/// The default content type when none is supplied.
pub const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";

/// Metadata for a stored object (everything but the bytes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMeta {
    pub key: String,
    pub size: usize,
    pub content_type: String,
    /// Content fingerprint (FNV-1a 64-bit, hex) — changes iff the bytes change.
    pub etag: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Errors returned by the object store.
#[derive(Debug, thiserror::Error)]
pub enum ObjectError {
    #[error("bucket '{0}' not found")]
    BucketNotFound(String),
    #[error("bucket '{0}' already exists")]
    BucketExists(String),
    #[error("object '{0}' not found")]
    ObjectNotFound(String),
    #[error("invalid bucket name '{0}'")]
    InvalidBucketName(String),
}

/// Content-addressed ETag: FNV-1a 64-bit over the bytes, as 16 hex digits.
pub fn etag_of(data: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// A bucket name must be non-empty and contain only `[a-z0-9.-]` (S3-ish).
pub fn valid_bucket_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
}
