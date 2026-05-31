//! Aegis Section-Compression Module
//!
//! Exposes `POST /api/v1/admin/compress` to recompress a cold section of data
//! with NexusCompress for a higher ratio. Supports the time-series paradigm:
//! cold Gorilla blocks of a metric within a time range are re-encoded as
//! NexusCompress frames. Reads stay transparent (each block is decoded by its
//! codec on query), and recompressed blocks are flushed to disk so they survive
//! a restart. Document-collection compression is a planned second step and for
//! now returns a clear "unsupported" response rather than silently doing
//! nothing.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::activity::ActivityType;
use crate::state::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

/// Request body for `POST /api/v1/admin/compress`.
///
/// Time-series form: `{ "metric": "NexusEdge", "start_ts": <ms>, "end_ts": <ms> }`
/// — omit `start_ts`/`end_ts` to compress the metric's whole history.
/// Document form: `{ "collection": "controllers" }` (not yet supported).
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
    // Document-collection compression is the planned second step. Be explicit
    // rather than silently no-op so callers know it's unsupported.
    if request.collection.is_some() && request.metric.is_none() {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(CompressResponse::err(
                "documents",
                "Document-collection compression is not yet supported; only the time-series paradigm is available.",
            )),
        );
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
