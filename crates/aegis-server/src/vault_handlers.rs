//! Aegis Vault API Handlers
//!
//! REST API handlers for the integrated secrets vault.

use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

// =============================================================================
// Request/Response Types
// =============================================================================

#[derive(Deserialize)]
pub struct UnsealRequest {
    pub passphrase: String,
}

#[derive(Deserialize)]
pub struct SetSecretRequest {
    pub value: String,
}

#[derive(Deserialize)]
pub struct TransitEncryptRequest {
    pub key_name: String,
    pub plaintext: String, // base64 encoded
}

#[derive(Deserialize)]
pub struct TransitDecryptRequest {
    pub key_name: String,
    pub ciphertext: String, // base64 encoded
}

#[derive(Deserialize)]
pub struct CreateTransitKeyRequest {
    pub name: String,
}

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub prefix: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct VaultStatusResponse {
    pub sealed: bool,
    pub secret_count: usize,
    pub transit_key_count: usize,
    pub uptime_secs: Option<u64>,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/v1/vault/status
pub async fn vault_status(State(state): State<AppState>) -> impl IntoResponse {
    let status = state.vault.status();
    Json(serde_json::json!({
        "sealed": status.sealed,
        "secret_count": status.secret_count,
        "transit_key_count": status.transit_key_count,
        "uptime_secs": status.uptime_secs,
    }))
}

/// POST /api/v1/vault/unseal
pub async fn vault_unseal(
    State(state): State<AppState>,
    Json(req): Json<UnsealRequest>,
) -> impl IntoResponse {
    match state.vault.unseal(&req.passphrase) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "unsealed"})),
        ),
        Err(e) => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// POST /api/v1/vault/seal
pub async fn vault_seal(State(state): State<AppState>) -> impl IntoResponse {
    match state.vault.seal() {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "sealed"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// GET /api/v1/vault/secrets
pub async fn list_secrets(
    State(state): State<AppState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let prefix = params.prefix.as_deref().unwrap_or("");
    match state.vault.list(prefix, "api") {
        Ok(keys) => (StatusCode::OK, Json(serde_json::json!({"keys": keys}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// GET /api/v1/vault/secrets/:key
pub async fn get_secret(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match state.vault.get(&key, "api") {
        Ok(value) => (
            StatusCode::OK,
            Json(serde_json::json!({"key": key, "value": value})),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// PUT /api/v1/vault/secrets/:key
pub async fn set_secret(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<SetSecretRequest>,
) -> impl IntoResponse {
    match state.vault.set(&key, &req.value, "api") {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "key": key})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// DELETE /api/v1/vault/secrets/:key
pub async fn delete_secret(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match state.vault.delete(&key, "api") {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "deleted", "key": key})),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// POST /api/v1/vault/transit/encrypt
pub async fn transit_encrypt(
    State(state): State<AppState>,
    Json(req): Json<TransitEncryptRequest>,
) -> impl IntoResponse {
    match state
        .vault
        .transit_encrypt(&req.key_name, req.plaintext.as_bytes())
    {
        Ok(ciphertext) => {
            let encoded = data_encoding_hex(&ciphertext);
            (
                StatusCode::OK,
                Json(serde_json::json!({"ciphertext": encoded})),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// POST /api/v1/vault/transit/decrypt
pub async fn transit_decrypt(
    State(state): State<AppState>,
    Json(req): Json<TransitDecryptRequest>,
) -> impl IntoResponse {
    match hex_decode(&req.ciphertext) {
        Ok(ciphertext) => match state.vault.transit_decrypt(&req.key_name, &ciphertext) {
            Ok(plaintext) => {
                let text = String::from_utf8_lossy(&plaintext).to_string();
                (StatusCode::OK, Json(serde_json::json!({"plaintext": text})))
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            ),
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("Invalid hex: {}", e)})),
        ),
    }
}

/// POST /api/v1/vault/transit/keys
pub async fn create_transit_key(
    State(state): State<AppState>,
    Json(req): Json<CreateTransitKeyRequest>,
) -> impl IntoResponse {
    match state.vault.transit_create_key(&req.name) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"status": "created", "key_name": req.name})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// GET /api/v1/vault/transit/keys
pub async fn list_transit_keys(State(state): State<AppState>) -> impl IntoResponse {
    let keys = state.vault.transit_list_keys();
    Json(serde_json::json!({"keys": keys}))
}

/// GET /api/v1/vault/audit
pub async fn vault_audit(
    State(state): State<AppState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);
    let entries = state.vault.audit_entries(limit);
    Json(serde_json::json!({"entries": entries}))
}

// =============================================================================
// Helpers
// =============================================================================

fn data_encoding_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("odd length".to_string());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
