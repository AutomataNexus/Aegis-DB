//! Aegis Shield API Handlers
//!
//! REST API handlers for the integrated security shield.

use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

// =============================================================================
// Request Types
// =============================================================================

#[derive(Deserialize, Default)]
pub struct EventsQuery {
    pub limit: Option<usize>,
    pub level: Option<String>,
}

#[derive(Deserialize)]
pub struct BlockIpRequest {
    pub ip: String,
    pub reason: Option<String>,
    pub duration_secs: Option<u64>,
}

#[derive(Deserialize)]
pub struct AllowlistRequest {
    pub ip: String,
}

#[derive(Deserialize)]
pub struct UpdatePolicyRequest {
    pub preset: Option<String>,
    pub sql_injection_enabled: Option<bool>,
    pub anomaly_detection_enabled: Option<bool>,
    pub ip_reputation_enabled: Option<bool>,
    pub fingerprinting_enabled: Option<bool>,
    pub auto_blocking_enabled: Option<bool>,
    pub auto_block_threshold: Option<u32>,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/v1/shield/status
pub async fn shield_status(State(state): State<AppState>) -> impl IntoResponse {
    let status = state.shield.get_status();
    Json(serde_json::json!({
        "enabled": status.enabled,
        "preset": format!("{:?}", status.preset),
        "uptime_secs": status.uptime_secs,
        "total_requests_analyzed": status.total_requests_analyzed,
        "total_threats_detected": status.total_threats_detected,
        "active_bans": status.active_bans,
        "blocked_ips": status.blocked_ips,
    }))
}

/// GET /api/v1/shield/stats
pub async fn shield_stats(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.shield.get_stats();
    Json(serde_json::to_value(&stats).unwrap_or_default())
}

/// GET /api/v1/shield/events
pub async fn shield_events(
    State(state): State<AppState>,
    Query(params): Query<EventsQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50);
    let events = state.shield.get_recent_events(limit);
    Json(serde_json::json!({"events": events, "count": events.len()}))
}

/// GET /api/v1/shield/blocked
pub async fn list_blocked(State(state): State<AppState>) -> impl IntoResponse {
    let blocked = state.shield.get_blocked_ips();
    Json(serde_json::json!({"blocked": blocked, "count": blocked.len()}))
}

/// POST /api/v1/shield/blocked
pub async fn block_ip(
    State(state): State<AppState>,
    Json(req): Json<BlockIpRequest>,
) -> impl IntoResponse {
    let reason = req.reason.unwrap_or_else(|| "Manual block".to_string());
    let duration = req.duration_secs.unwrap_or(3600);
    state.shield.manual_block(&req.ip, &reason, duration);
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "status": "blocked",
            "ip": req.ip,
            "duration_secs": duration,
        })),
    )
}

/// DELETE /api/v1/shield/blocked/:ip
pub async fn unblock_ip(
    State(state): State<AppState>,
    Path(ip): Path<String>,
) -> impl IntoResponse {
    if state.shield.unblock_ip(&ip) {
        (
            StatusCode::OK,
            Json(serde_json::json!({"status": "unblocked", "ip": ip})),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "IP not found in block list"})),
        )
    }
}

/// GET /api/v1/shield/allowlist
pub async fn get_allowlist(State(state): State<AppState>) -> impl IntoResponse {
    let list = state.shield.get_allowlist();
    Json(serde_json::json!({"allowlist": list}))
}

/// POST /api/v1/shield/allowlist
pub async fn add_to_allowlist(
    State(state): State<AppState>,
    Json(req): Json<AllowlistRequest>,
) -> impl IntoResponse {
    state.shield.add_to_allowlist(&req.ip);
    (
        StatusCode::CREATED,
        Json(serde_json::json!({"status": "added", "ip": req.ip})),
    )
}

/// DELETE /api/v1/shield/allowlist/:ip
pub async fn remove_from_allowlist(
    State(state): State<AppState>,
    Path(ip): Path<String>,
) -> impl IntoResponse {
    state.shield.remove_from_allowlist(&ip);
    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "removed", "ip": ip})),
    )
}

/// GET /api/v1/shield/policy
pub async fn get_policy(State(state): State<AppState>) -> impl IntoResponse {
    let policy = state.shield.get_policy();
    Json(serde_json::to_value(&policy).unwrap_or_default())
}

/// PUT /api/v1/shield/policy
pub async fn update_policy(
    State(state): State<AppState>,
    Json(req): Json<UpdatePolicyRequest>,
) -> impl IntoResponse {
    let mut policy = state.shield.get_policy();

    if let Some(preset_str) = &req.preset {
        match preset_str.to_lowercase().as_str() {
            "strict" => {
                policy =
                    aegis_shield::SecurityPolicy::from_preset(aegis_shield::SecurityPreset::Strict)
            }
            "moderate" => {
                policy = aegis_shield::SecurityPolicy::from_preset(
                    aegis_shield::SecurityPreset::Moderate,
                )
            }
            "permissive" => {
                policy = aegis_shield::SecurityPolicy::from_preset(
                    aegis_shield::SecurityPreset::Permissive,
                )
            }
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Invalid preset. Use: strict, moderate, permissive"
                    })),
                );
            }
        }
    }

    if let Some(v) = req.sql_injection_enabled {
        policy.sql_injection_enabled = v;
    }
    if let Some(v) = req.anomaly_detection_enabled {
        policy.anomaly_detection_enabled = v;
    }
    if let Some(v) = req.ip_reputation_enabled {
        policy.ip_reputation_enabled = v;
    }
    if let Some(v) = req.fingerprinting_enabled {
        policy.fingerprinting_enabled = v;
    }
    if let Some(v) = req.auto_blocking_enabled {
        policy.auto_blocking_enabled = v;
    }
    state.shield.update_policy(policy);
    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "updated"})),
    )
}

/// GET /api/v1/shield/ip/:ip
pub async fn get_ip_reputation(
    State(state): State<AppState>,
    Path(ip): Path<String>,
) -> impl IntoResponse {
    match state.shield.get_ip_reputation(&ip) {
        Some(rep) => (
            StatusCode::OK,
            Json(serde_json::to_value(&rep).unwrap_or_default()),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "No data for this IP"})),
        ),
    }
}

/// GET /api/v1/shield/feed
pub async fn shield_feed(State(state): State<AppState>) -> impl IntoResponse {
    let stats = state.shield.get_stats();
    let recent = state.shield.get_recent_events(10);
    Json(serde_json::json!({
        "stats": stats,
        "recent_events": recent,
    }))
}
