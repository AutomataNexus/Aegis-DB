//! Aegis Section-Compression Module
//!
//! Exposes `POST /api/v1/admin/compress` to recompress a section of data with
//! NexusCompress for a higher ratio. Two paradigms:
//!   - Time-series (`{metric,start_ts,end_ts}`): cold Gorilla blocks in the range
//!     are re-encoded as NexusCompress frames. Reads stay transparent (each block
//!     is decoded by its codec on query); recompressed blocks are flushed so they
//!     survive a restart.
//!   - Documents (`{collection}`): the collection's at-rest file is rewritten as a
//!     compressed NexusCompress `Record` frame. Doc collections are also stored
//!     compressed automatically on flush; the loader reads frame-or-legacy-JSON
//!     transparently via the `NCZL` magic.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::activity::ActivityType;
use crate::state::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use nexuscompress::core_types::{DType, Domain, Schema};
use nexuscompress::record::RecordCompressor;
use serde::{Deserialize, Serialize};

// ── Document-collection compression (NexusCompress, Record domain) ─────────
// A collection persists as `documents/<name>.json` (a JSON array). To make that
// at-rest representation compressed yet transparently readable, the array is
// serialized and stored as a single `Bytes` column inside a self-describing
// NexusCompress frame (`Record` domain → zstd). The frame leads with the `NCZL`
// magic, so the loader distinguishes a compressed file from a legacy plain-JSON
// file by sniffing the first bytes — no filename/format flag and no migration:
// legacy `.json` files keep loading, and the next flush rewrites them compressed.

/// Leading bytes of a serialized NexusCompress frame (`NCZL`).
pub const NCZL_MAGIC: [u8; 4] = [0x4E, 0x43, 0x5A, 0x4C];

/// Single-`Bytes`-column schema holding a collection's serialized JSON array.
fn docs_schema() -> Schema {
    Schema::builder()
        .type_name("AegisDocCollection")
        .domain(Domain::Record)
        .field("json", DType::Bytes)
        .build()
}

/// Compress a collection's documents into a NexusCompress frame.
pub fn compress_docs(docs: &[serde_json::Value]) -> Result<Vec<u8>, String> {
    let json = serde_json::to_vec(docs).map_err(|e| e.to_string())?;
    let frame = RecordCompressor::from_schema(docs_schema())
        .compress_batch(&[json], docs.len() as u64)
        .map_err(|e| format!("{e:?}"))?;
    Ok(frame)
}

/// Encode a collection for at-rest storage: a NexusCompress frame when it is
/// actually smaller than the raw JSON, otherwise the raw JSON (so tiny
/// collections aren't bloated by frame overhead). Both forms are read back
/// transparently by `decompress_docs`. Returns `(bytes_to_write, raw_json_len)`.
pub fn encode_docs(docs: &[serde_json::Value]) -> (Vec<u8>, usize) {
    let raw = serde_json::to_vec(docs).unwrap_or_else(|_| b"[]".to_vec());
    match compress_docs(docs) {
        Ok(frame) if frame.len() < raw.len() => {
            let raw_len = raw.len();
            (frame, raw_len)
        }
        _ => {
            let raw_len = raw.len();
            (raw, raw_len)
        }
    }
}

/// Decode collection bytes that are EITHER a NexusCompress frame OR a legacy
/// plain-JSON array. Returns `None` only if the bytes are neither (corrupt).
pub fn decompress_docs(bytes: &[u8]) -> Option<Vec<serde_json::Value>> {
    if bytes.len() >= 4 && bytes[0..4] == NCZL_MAGIC {
        let cols = RecordCompressor::from_schema(docs_schema())
            .decompress_batch(bytes)
            .ok()?;
        let json = cols.into_iter().next()?;
        serde_json::from_slice::<Vec<serde_json::Value>>(&json).ok()
    } else {
        serde_json::from_slice::<Vec<serde_json::Value>>(bytes).ok()
    }
}

/// Request body for `POST /api/v1/admin/compress`.
///
/// Time-series form: `{ "metric": "NexusEdge", "start_ts": <ms>, "end_ts": <ms> }`
/// — omit `start_ts`/`end_ts` to compress the metric's whole history.
/// Document form: `{ "collection": "controllers" }`.
#[derive(Debug, Deserialize)]
pub struct CompressRequest {
    #[serde(default)]
    pub metric: Option<String>,
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub start_ts: Option<i64>,
    #[serde(default)]
    pub end_ts: Option<i64>,
}

/// Response for `POST /api/v1/admin/compress`.
#[derive(Debug, Serialize)]
pub struct CompressResponse {
    pub success: bool,
    pub paradigm: String,
    pub blocks_scanned: usize,
    pub blocks_recompressed: usize,
    pub blocks_already: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub error: Option<String>,
}

impl CompressResponse {
    fn err(paradigm: &str, msg: impl Into<String>) -> Self {
        Self {
            success: false,
            paradigm: paradigm.to_string(),
            blocks_scanned: 0,
            blocks_recompressed: 0,
            blocks_already: 0,
            bytes_before: 0,
            bytes_after: 0,
            error: Some(msg.into()),
        }
    }
}

/// Recompress a cold section of data with NexusCompress.
pub async fn compress_section(
    State(state): State<AppState>,
    Json(request): Json<CompressRequest>,
) -> impl IntoResponse {
    // Document-collection compression: rewrite the collection's at-rest file as
    // a compressed NexusCompress frame and report the size delta. (Collections
    // are also compressed-at-rest automatically on flush; this forces it now.)
    if let (Some(collection), None) = (request.collection.as_ref(), request.metric.as_ref()) {
        state.activity.log(
            ActivityType::System,
            &format!("Compressing document collection '{}'", collection),
        );
        return match state.compress_collection(collection) {
            Some((docs, before, after)) => {
                state.activity.log(
                    ActivityType::System,
                    &format!("Compressed collection '{collection}': {before} -> {after} bytes"),
                );
                (
                    StatusCode::OK,
                    Json(CompressResponse {
                        success: true,
                        paradigm: "documents".to_string(),
                        blocks_scanned: docs,
                        blocks_recompressed: docs,
                        blocks_already: 0,
                        bytes_before: before,
                        bytes_after: after,
                        error: None,
                    }),
                )
            }
            None => (
                StatusCode::NOT_FOUND,
                Json(CompressResponse::err(
                    "documents",
                    "Collection not found or no data directory configured.",
                )),
            ),
        };
    }

    let metric = request.metric.unwrap_or_default();
    let start_ms = request.start_ts.unwrap_or(i64::MIN);
    let end_ms = request.end_ts.unwrap_or(i64::MAX);

    state.activity.log(
        ActivityType::System,
        &format!(
            "Compressing time-series section (metric='{}', {}..{})",
            metric, start_ms, end_ms
        ),
    );

    let report = state
        .timeseries_engine
        .compress_section(&metric, start_ms, end_ms);

    // Persist so recompressed blocks survive a restart.
    if let Err(e) = state.save_to_disk() {
        tracing::warn!("compress: failed to persist after recompression: {}", e);
    }

    state.activity.log(
        ActivityType::System,
        &format!(
            "Compressed {} block(s): {} -> {} bytes",
            report.blocks_recompressed, report.bytes_before, report.bytes_after
        ),
    );

    (
        StatusCode::OK,
        Json(CompressResponse {
            success: true,
            paradigm: "timeseries".to_string(),
            blocks_scanned: report.blocks_scanned,
            blocks_recompressed: report.blocks_recompressed,
            blocks_already: report.blocks_already,
            bytes_before: report.bytes_before,
            bytes_after: report.bytes_after,
            error: None,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Vec<serde_json::Value> {
        (0..200)
            .map(|i| {
                json!({
                    "_id": format!("controller-{i}"),
                    "controller_name": format!("Unit {i}"),
                    "organization_id": "acme",
                    "equipment_count": i % 9,
                    "online": i % 2 == 0,
                    "tags": ["hvac", "rtu", "site-a"],
                })
            })
            .collect()
    }

    #[test]
    fn docs_compress_roundtrip_and_magic() {
        let docs = sample();
        let frame = compress_docs(&docs).expect("compress");
        // Frame is a NexusCompress frame and is smaller than the raw JSON.
        assert_eq!(frame[0..4], NCZL_MAGIC);
        let raw = serde_json::to_vec(&docs).unwrap();
        assert!(frame.len() < raw.len(), "expected compression to shrink");
        // Round-trips back to the exact documents.
        let back = decompress_docs(&frame).expect("decompress frame");
        assert_eq!(back, docs);
    }

    #[test]
    fn decompress_reads_legacy_plain_json() {
        // A legacy (uncompressed) collection file must still load.
        let docs = sample();
        let plain = serde_json::to_vec(&docs).unwrap();
        assert_ne!(plain[0..4], NCZL_MAGIC);
        let back = decompress_docs(&plain).expect("decompress legacy json");
        assert_eq!(back, docs);
    }

    #[test]
    fn empty_and_corrupt() {
        let frame = compress_docs(&[]).expect("compress empty");
        assert_eq!(
            decompress_docs(&frame).expect("empty roundtrip"),
            Vec::<serde_json::Value>::new()
        );
        // Garbage that is neither a frame nor JSON yields None (caller skips it).
        assert!(decompress_docs(b"not json and not a frame").is_none());
    }

    #[test]
    fn encode_docs_picks_smaller_form() {
        // Tiny collection: frame overhead loses, so raw JSON is stored (no bloat),
        // and still round-trips.
        let tiny = vec![json!({"a": 1})];
        let (bytes_tiny, raw_tiny) = encode_docs(&tiny);
        assert_ne!(bytes_tiny[0..4], NCZL_MAGIC, "tiny should stay raw JSON");
        assert_eq!(bytes_tiny.len(), raw_tiny);
        assert_eq!(decompress_docs(&bytes_tiny).unwrap(), tiny);

        // Large/repetitive collection: the frame wins, so it's stored compressed.
        let big = sample();
        let (bytes_big, raw_big) = encode_docs(&big);
        assert_eq!(bytes_big[0..4], NCZL_MAGIC, "big should compress");
        assert!(bytes_big.len() < raw_big);
        assert_eq!(decompress_docs(&bytes_big).unwrap(), big);
    }
}
