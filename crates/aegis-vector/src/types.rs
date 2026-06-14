//! Core types for the vector engine: distance metrics, records, errors.

use serde::{Deserialize, Serialize};

/// Distance metric for nearest-neighbor search.
///
/// All metrics expose a `distance` where **lower means closer**, so the same
/// min-heap search logic works for every metric. For `Cosine` and `Dot` (where
/// a *higher* similarity means closer) the distance is the negated similarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Metric {
    /// Cosine distance. Vectors are L2-normalized on insert, so this reduces to
    /// `1 - dot(a, b)` over unit vectors.
    Cosine,
    /// Squared Euclidean (L2) distance — monotonic with L2, cheaper (no sqrt).
    L2,
    /// Negative inner product (dot). Higher dot ⇒ smaller distance.
    Dot,
}

impl Metric {
    /// Parse from a string (`"cosine"`, `"l2"`/`"euclidean"`, `"dot"`/`"ip"`).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "cosine" | "cos" => Some(Metric::Cosine),
            "l2" | "euclidean" | "euclid" => Some(Metric::L2),
            "dot" | "ip" | "inner_product" | "inner-product" => Some(Metric::Dot),
            _ => None,
        }
    }

    /// Distance between two equal-length vectors (lower = closer).
    #[inline]
    pub fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            Metric::L2 => {
                let mut s = 0.0f32;
                for i in 0..a.len() {
                    let d = a[i] - b[i];
                    s += d * d;
                }
                s
            }
            // For Cosine we assume both vectors are already normalized, so the
            // cosine similarity is just the dot product; distance = 1 - sim.
            Metric::Cosine => 1.0 - dot(a, b),
            Metric::Dot => -dot(a, b),
        }
    }

    /// Whether vectors should be L2-normalized before being stored/indexed.
    pub fn normalizes(&self) -> bool {
        matches!(self, Metric::Cosine)
    }
}

/// Dot product of two equal-length slices.
#[inline]
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut s = 0.0f32;
    for i in 0..a.len() {
        s += a[i] * b[i];
    }
    s
}

/// L2-normalize a vector in place. A zero vector is left unchanged.
pub fn normalize(v: &mut [f32]) {
    let norm = dot(v, v).sqrt();
    if norm > 0.0 {
        let inv = 1.0 / norm;
        for x in v.iter_mut() {
            *x *= inv;
        }
    }
}

/// A stored vector with an opaque id and arbitrary JSON metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorRecord {
    pub id: String,
    pub vector: Vec<f32>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// A single search result: the record id, its score, and its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: String,
    /// Similarity-style score where **higher = more similar** (cosine/dot
    /// similarity, or negative squared-L2 distance), regardless of metric.
    pub score: f32,
    /// Raw metric distance (lower = closer) as used internally.
    pub distance: f32,
    pub metadata: serde_json::Value,
}

/// Errors returned by the vector engine.
#[derive(Debug, thiserror::Error)]
pub enum VectorError {
    #[error("collection '{0}' not found")]
    CollectionNotFound(String),
    #[error("collection '{0}' already exists")]
    CollectionExists(String),
    #[error("vector dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("invalid dimension: must be > 0")]
    InvalidDimension,
    #[error("unknown metric '{0}' (use cosine, l2, or dot)")]
    UnknownMetric(String),
    #[error("vector '{0}' not found")]
    VectorNotFound(String),
}
