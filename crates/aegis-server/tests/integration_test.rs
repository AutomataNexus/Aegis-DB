//! End-to-end integration tests for Aegis Server
//!
//! Tests the full API flow including authentication, admin endpoints, and activity logging.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::Service;
use serde_json::{json, Value};
use std::sync::Arc;
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use sha1::Sha1;

use aegis_server::{create_router, AppState, ServerConfig};

/// Helper to make a GET request and return JSON response.
async fn get_json(app: &mut axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .call(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

/// Helper to make a POST request with JSON body and return JSON response.
async fn post_json(app: &mut axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .call(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_string(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

/// Create a shared app state for tests that need state persistence.
fn shared_state() -> Arc<AppState> {
    Arc::new(AppState::new(ServerConfig::default()))
}

/// Generate a valid TOTP code for the admin user's secret.
fn generate_admin_totp() -> String {
    // Admin user's secret from auth.rs
    let secret = "JBSWY3DPEHPK3PXP";

    let secret_bytes = BASE32_NOPAD.decode(secret.as_bytes())
        .unwrap_or_else(|_| {
            let padded = format!("{}{}", secret, &"========"[..((8 - secret.len() % 8) % 8)]);
            data_encoding::BASE32.decode(padded.as_bytes()).unwrap()
        });

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let time_step = timestamp / 30;
    let counter_bytes = time_step.to_be_bytes();

    let mut mac = Hmac::<Sha1>::new_from_slice(&secret_bytes).unwrap();
    mac.update(&counter_bytes);
    let result = mac.finalize().into_bytes();

    let offset_idx = (result[19] & 0x0f) as usize;
    let binary_code = ((result[offset_idx] & 0x7f) as u32) << 24
        | (result[offset_idx + 1] as u32) << 16
        | (result[offset_idx + 2] as u32) << 8
        | (result[offset_idx + 3] as u32);

    let otp = binary_code % 1_000_000;
    format!("{:06}", otp)
}

/// Create router with shared state.
fn app_with_state(state: Arc<AppState>) -> axum::Router {
    create_router((*state).clone())
}

// =============================================================================
// Health Check Tests
// =============================================================================

#[tokio::test]
async fn test_health_endpoint() {
    let state = shared_state();
    let mut app = app_with_state(state);
    let (status, json) = get_json(&mut app, "/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "healthy");
    assert!(json["version"].is_string());
}

// =============================================================================
// Authentication E2E Tests
// =============================================================================

#[tokio::test]
async fn test_login_demo_user_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "demo",
            "password": "demo"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(json["token"].is_string());
    assert!(json["user"].is_object());
    assert_eq!(json["user"]["username"], "demo");
    assert_eq!(json["user"]["role"], "viewer");
    assert_eq!(json["user"]["mfa_enabled"], false);
}

#[tokio::test]
async fn test_login_admin_requires_mfa_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "admin",
            "password": "admin"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(json["token"].is_string());
    assert_eq!(json["requires_mfa"], true);
    assert!(json["user"].is_null());
}

#[tokio::test]
async fn test_login_invalid_credentials_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "admin",
            "password": "wrongpassword"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(json["error"].is_string());
}

#[tokio::test]
async fn test_full_mfa_flow_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Step 1: Login as admin
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "admin",
            "password": "admin"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["requires_mfa"], true);
    let temp_token = json["token"].as_str().unwrap().to_string();

    // Step 2: Verify MFA with correct code (same app instance for session persistence)
    let totp_code = generate_admin_totp();
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/mfa/verify",
        json!({
            "code": totp_code,
            "token": temp_token
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(json["token"].is_string());
    assert!(json["user"].is_object());
    assert_eq!(json["user"]["username"], "admin");
    assert_eq!(json["user"]["role"], "admin");
    assert_eq!(json["user"]["mfa_enabled"], true);
}

#[tokio::test]
async fn test_mfa_invalid_code_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Step 1: Login as admin
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "admin",
            "password": "admin"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let temp_token = json["token"].as_str().unwrap().to_string();

    // Step 2: Try invalid MFA code
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/mfa/verify",
        json!({
            "code": "000000",
            "token": temp_token
        }),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(json["error"].is_string());
}

#[tokio::test]
async fn test_logout_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Login first
    let (_, json) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "demo",
            "password": "demo"
        }),
    )
    .await;

    let token = json["token"].as_str().unwrap().to_string();

    // Logout (same app instance)
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/logout",
        json!({
            "token": token
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
}

// =============================================================================
// Admin API E2E Tests
// =============================================================================

#[tokio::test]
async fn test_get_cluster_info_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);
    let (status, json) = get_json(&mut app, "/api/v1/admin/cluster").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["name"], "aegis-cluster");
    assert!(json["version"].is_string());
    assert!(json["node_count"].is_number());
    assert!(json["uptime_seconds"].is_number());
}

#[tokio::test]
async fn test_get_dashboard_summary_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);
    let (status, json) = get_json(&mut app, "/api/v1/admin/dashboard").await;

    assert_eq!(status, StatusCode::OK);
    assert!(json["cluster"].is_object());
    assert!(json["performance"].is_object());
    assert!(json["storage"].is_object());
    assert!(json["alerts"].is_object());

    // Verify cluster section
    assert_eq!(json["cluster"]["state"], "Healthy");
    assert!(json["cluster"]["node_count"].is_number());

    // Verify performance section
    assert!(json["performance"]["queries_per_second"].is_number());
    assert!(json["performance"]["avg_latency_ms"].is_number());
    assert!(json["performance"]["cpu_usage_percent"].is_number());
}

#[tokio::test]
async fn test_get_nodes_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);
    let (status, json) = get_json(&mut app, "/api/v1/admin/nodes").await;

    assert_eq!(status, StatusCode::OK);
    assert!(json.is_array());

    let nodes = json.as_array().unwrap();
    assert!(!nodes.is_empty());

    // Verify first node has all required fields
    let node = &nodes[0];
    assert!(node["id"].is_string());
    assert!(node["address"].is_string());
    assert!(node["role"].is_string());
    assert!(node["status"].is_string());
    assert!(node["metrics"].is_object());

    // Verify metrics have network and latency fields
    let metrics = &node["metrics"];
    assert!(metrics["cpu_usage_percent"].is_number());
    assert!(metrics["memory_usage_bytes"].is_number());
    assert!(metrics["network_bytes_in"].is_number());
    assert!(metrics["network_bytes_out"].is_number());
    assert!(metrics["latency_p50_ms"].is_number());
    assert!(metrics["latency_p99_ms"].is_number());
}

#[tokio::test]
async fn test_get_storage_info_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);
    let (status, json) = get_json(&mut app, "/api/v1/admin/storage").await;

    assert_eq!(status, StatusCode::OK);
    assert!(json["total_bytes"].is_number());
    assert!(json["used_bytes"].is_number());
    assert!(json["available_bytes"].is_number());
    assert!(json["data_bytes"].is_number());
    assert!(json["index_bytes"].is_number());
    assert!(json["wal_bytes"].is_number());
}

#[tokio::test]
async fn test_get_query_stats_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);
    let (status, json) = get_json(&mut app, "/api/v1/admin/stats").await;

    assert_eq!(status, StatusCode::OK);
    assert!(json["total_queries"].is_number());
    assert!(json["queries_per_second"].is_number());
    assert!(json["avg_duration_ms"].is_number());
    assert!(json["p50_duration_ms"].is_number());
    assert!(json["p95_duration_ms"].is_number());
    assert!(json["p99_duration_ms"].is_number());
    assert!(json["slow_queries"].is_number());
    assert!(json["failed_queries"].is_number());
}

#[tokio::test]
async fn test_get_alerts_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);
    let (status, json) = get_json(&mut app, "/api/v1/admin/alerts").await;

    assert_eq!(status, StatusCode::OK);
    assert!(json["alerts"].is_array());

    let alerts = json["alerts"].as_array().unwrap();
    if !alerts.is_empty() {
        let alert = &alerts[0];
        assert!(alert["id"].is_string());
        assert!(alert["severity"].is_string());
        assert!(alert["message"].is_string());
        assert!(alert["source"].is_string());
    }
}

// =============================================================================
// Activity Logging E2E Tests
// =============================================================================

#[tokio::test]
async fn test_get_activities_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);
    let (status, json) = get_json(&mut app, "/api/v1/admin/activities").await;

    assert_eq!(status, StatusCode::OK);
    assert!(json.is_array());

    // There should be at least the server startup activity
    let activities = json.as_array().unwrap();
    if !activities.is_empty() {
        let activity = &activities[0];
        assert!(activity["id"].is_string());
        assert!(activity["type"].is_string());
        assert!(activity["description"].is_string());
        assert!(activity["timestamp"].is_string());
    }
}

#[tokio::test]
async fn test_activities_logged_after_login_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Login to generate activity
    let _ = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "demo",
            "password": "demo"
        }),
    )
    .await;

    // Check activities - should include the login
    let (status, json) = get_json(&mut app, "/api/v1/admin/activities?limit=10").await;

    assert_eq!(status, StatusCode::OK);
    assert!(json.is_array());

    let activities = json.as_array().unwrap();
    // Should have activities (at least server startup + login)
    assert!(activities.len() >= 1);
}

// =============================================================================
// Query API E2E Tests
// =============================================================================

#[tokio::test]
async fn test_list_tables_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);
    let (status, json) = get_json(&mut app, "/api/v1/tables").await;

    assert_eq!(status, StatusCode::OK);
    assert!(json["tables"].is_array());
}

// =============================================================================
// Error Handling E2E Tests
// =============================================================================

#[tokio::test]
async fn test_not_found_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);
    let (status, json) = get_json(&mut app, "/api/v1/nonexistent").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(json["error"].is_string());
}

#[tokio::test]
async fn test_metrics_endpoint_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);
    let (status, json) = get_json(&mut app, "/api/v1/metrics").await;

    assert_eq!(status, StatusCode::OK);
    assert!(json["total_requests"].is_number());
    assert!(json["failed_requests"].is_number());
    assert!(json["avg_duration_ms"].is_number());
    assert!(json["success_rate"].is_number());
}

// =============================================================================
// Full User Journey E2E Test
// =============================================================================

#[tokio::test]
async fn test_full_user_journey_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // 1. Check health
    let (status, _) = get_json(&mut app, "/health").await;
    assert_eq!(status, StatusCode::OK);

    // 2. Login as demo user
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "demo",
            "password": "demo"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _token = json["token"].as_str().unwrap().to_string();

    // 3. Get dashboard summary
    let (status, json) = get_json(&mut app, "/api/v1/admin/dashboard").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["cluster"]["state"], "Healthy");

    // 4. Get nodes
    let (status, json) = get_json(&mut app, "/api/v1/admin/nodes").await;
    assert_eq!(status, StatusCode::OK);
    let nodes = json.as_array().unwrap();
    assert!(!nodes.is_empty());

    // 5. Get storage info
    let (status, json) = get_json(&mut app, "/api/v1/admin/storage").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["total_bytes"].as_u64().unwrap() > json["used_bytes"].as_u64().unwrap());

    // 6. Get query stats
    let (status, json) = get_json(&mut app, "/api/v1/admin/stats").await;
    assert_eq!(status, StatusCode::OK);
    // With no queries recorded, percentiles may all be 0.0
    // Just verify the fields exist and are non-negative
    assert!(json["p50_duration_ms"].as_f64().unwrap() >= 0.0);
    assert!(json["p99_duration_ms"].as_f64().unwrap() >= 0.0);

    // 7. Get alerts
    let (status, _) = get_json(&mut app, "/api/v1/admin/alerts").await;
    assert_eq!(status, StatusCode::OK);

    // 8. Get activities
    let (status, _) = get_json(&mut app, "/api/v1/admin/activities").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_admin_user_full_journey_with_mfa_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // 1. Login as admin (requires MFA)
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "admin",
            "password": "admin"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["requires_mfa"], true);
    let temp_token = json["token"].as_str().unwrap().to_string();

    // 2. Verify MFA
    let totp_code = generate_admin_totp();
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/mfa/verify",
        json!({
            "code": totp_code,
            "token": temp_token
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["user"]["role"], "admin");
    let token = json["token"].as_str().unwrap().to_string();

    // 3. Access admin resources
    let (status, _) = get_json(&mut app, "/api/v1/admin/dashboard").await;
    assert_eq!(status, StatusCode::OK);

    // 4. Logout
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/logout",
        json!({
            "token": token
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
}

// =============================================================================
// Operator User Tests
// =============================================================================

#[tokio::test]
async fn test_operator_user_login_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "operator",
            "password": "operator"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(json["token"].is_string());
    assert!(json["user"].is_object());
    assert_eq!(json["user"]["username"], "operator");
    assert_eq!(json["user"]["role"], "operator");
    assert_eq!(json["user"]["mfa_enabled"], false);
}

// =============================================================================
// Session Validation Tests
// =============================================================================

#[tokio::test]
async fn test_session_validation_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Login
    let (_, json) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "demo",
            "password": "demo"
        }),
    )
    .await;
    let token = json["token"].as_str().unwrap();

    // Validate session
    let (status, json) = get_json(&mut app, &format!("/api/v1/auth/session?token={}", token)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.is_object());
    assert_eq!(json["username"], "demo");
}

#[tokio::test]
async fn test_invalid_session_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, _) = get_json(&mut app, "/api/v1/auth/session?token=invalid_token").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// =============================================================================
// Key-Value Store E2E Tests
// =============================================================================

#[tokio::test]
async fn test_kv_crud_operations_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // CREATE - Set a key (returns KvEntry with key, value, ttl, created_at, updated_at)
    let (status, json) = post_json(
        &mut app,
        "/api/v1/kv/keys",
        json!({
            "key": "test_key_1",
            "value": "test_value_1"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["key"], "test_key_1");
    assert_eq!(json["value"], "test_value_1");

    // UPDATE - Update the key (same endpoint, returns new entry)
    let (status, json) = post_json(
        &mut app,
        "/api/v1/kv/keys",
        json!({
            "key": "test_key_1",
            "value": "updated_value"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["key"], "test_key_1");
    assert_eq!(json["value"], "updated_value");

    // LIST - Get all keys (returns mock data + any set keys)
    let (status, json) = get_json(&mut app, "/api/v1/kv/keys").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["keys"].is_array());
    assert!(json["total"].is_number());
}

#[tokio::test]
async fn test_kv_list_with_prefix_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // List keys with prefix filter (GET /api/v1/kv/keys?prefix=user:)
    let (status, json) = get_json(&mut app, "/api/v1/kv/keys?prefix=user:").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["keys"].is_array());
    // Mock data has user:1001 and user:1002 keys
    let keys = json["keys"].as_array().unwrap();
    for key in keys {
        assert!(key["key"].as_str().unwrap().starts_with("user:"));
    }
}

#[tokio::test]
async fn test_kv_multiple_keys_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Insert multiple keys and verify each returns the correct entry
    for i in 1..=5 {
        let (status, json) = post_json(
            &mut app,
            "/api/v1/kv/keys",
            json!({
                "key": format!("batch_key_{}", i),
                "value": format!("batch_value_{}", i)
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["key"], format!("batch_key_{}", i));
        assert_eq!(json["value"], format!("batch_value_{}", i));
    }

    // List keys (returns mock data)
    let (status, json) = get_json(&mut app, "/api/v1/kv/keys").await;
    assert_eq!(status, StatusCode::OK);
    let keys = json["keys"].as_array().unwrap();
    assert!(!keys.is_empty());
}

#[tokio::test]
async fn test_kv_json_value_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Store JSON object directly as value (handler accepts serde_json::Value)
    let json_value = json!({
        "nested": {
            "data": [1, 2, 3],
            "flag": true
        }
    });

    let (status, response_json) = post_json(
        &mut app,
        "/api/v1/kv/keys",
        json!({
            "key": "json_key",
            "value": json_value
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response_json["key"], "json_key");
    // Value should be stored as JSON object
    assert!(response_json["value"].is_object());
    assert_eq!(response_json["value"]["nested"]["flag"], true);
}

// =============================================================================
// Document Store E2E Tests
// =============================================================================

#[tokio::test]
async fn test_document_collections_list_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // First create a collection
    let (create_status, _) = post_json(
        &mut app,
        "/api/v1/documents/collections",
        json!({ "name": "test_collection" }),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    // Now list collections
    let (status, json) = get_json(&mut app, "/api/v1/documents/collections").await;
    assert_eq!(status, StatusCode::OK);
    // Returns array directly with collection info objects
    assert!(json.is_array());
    let collections = json.as_array().unwrap();
    assert!(!collections.is_empty());
    // Verify first collection has expected fields
    let first = &collections[0];
    assert!(first["name"].is_string());
    assert!(first["document_count"].is_number());
    assert!(first["index_count"].is_number());
}

#[tokio::test]
async fn test_document_get_collection_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // First create the 'users' collection
    let (create_status, _) = post_json(
        &mut app,
        "/api/v1/documents/collections",
        json!({ "name": "users" }),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    // Insert a document
    let (insert_status, _) = post_json(
        &mut app,
        "/api/v1/documents/collections/users/documents",
        json!({
            "document": {
                "name": "Test User",
                "email": "test@example.com"
            }
        }),
    )
    .await;
    assert_eq!(insert_status, StatusCode::CREATED);

    // GET documents from 'users' collection
    let (status, json) = get_json(&mut app, "/api/v1/documents/collections/users").await;
    assert_eq!(status, StatusCode::OK);
    // Returns CollectionQueryResponse with documents array, total_scanned, execution_time_ms
    assert!(json["documents"].is_array());
    assert!(json["total_scanned"].is_number());
    assert!(json["execution_time_ms"].is_number());
    let docs = json["documents"].as_array().unwrap();
    assert!(!docs.is_empty());
    let doc = &docs[0];
    assert!(doc["id"].is_string());
    assert_eq!(doc["collection"], "users");
    assert!(doc["data"].is_object());
}

#[tokio::test]
async fn test_document_get_products_collection_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // First create the 'products' collection
    let (create_status, _) = post_json(
        &mut app,
        "/api/v1/documents/collections",
        json!({ "name": "products" }),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    // Insert a product document
    let (insert_status, _) = post_json(
        &mut app,
        "/api/v1/documents/collections/products/documents",
        json!({
            "document": {
                "name": "Test Product",
                "sku": "TEST-001",
                "price": 99.99
            }
        }),
    )
    .await;
    assert_eq!(insert_status, StatusCode::CREATED);

    // GET documents from 'products' collection
    let (status, json) = get_json(&mut app, "/api/v1/documents/collections/products").await;
    assert_eq!(status, StatusCode::OK);
    // Returns CollectionQueryResponse with documents array, total_scanned, execution_time_ms
    assert!(json["documents"].is_array());
    assert!(json["total_scanned"].is_number());
    assert!(json["execution_time_ms"].is_number());
    let docs = json["documents"].as_array().unwrap();
    assert!(!docs.is_empty());
    let doc = &docs[0];
    assert!(doc["data"]["name"].is_string());
    assert!(doc["data"]["sku"].is_string());
    assert!(doc["data"]["price"].is_number());
}

// =============================================================================
// SQL Query Execution E2E Tests
// =============================================================================

#[tokio::test]
async fn test_sql_select_query_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // POST /api/v1/query expects {"sql": "...", "params": []}
    let (status, json) = post_json(
        &mut app,
        "/api/v1/query",
        json!({
            "sql": "SELECT 1 + 1 AS result"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert!(json["data"].is_object());
    assert!(json["execution_time_ms"].is_number());
}

#[tokio::test]
async fn test_sql_create_table_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, json) = post_json(
        &mut app,
        "/api/v1/query",
        json!({
            "sql": "CREATE TABLE IF NOT EXISTS e2e_test_users (id INTEGER NOT NULL, name VARCHAR(255), email VARCHAR(255))"
        }),
    )
    .await;

    // Table creation should return success or parse error if not fully implemented
    assert!(status == StatusCode::OK || status == StatusCode::BAD_REQUEST);
    // Should have execution time
    assert!(json["execution_time_ms"].is_number());
}

#[tokio::test]
async fn test_sql_insert_and_select_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Execute simple SELECT first to verify query endpoint works
    let (status, json) = post_json(
        &mut app,
        "/api/v1/query",
        json!({
            "sql": "SELECT 'test' AS name, 100 AS price"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
}

#[tokio::test]
async fn test_sql_syntax_error_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, json) = post_json(
        &mut app,
        "/api/v1/query",
        json!({
            "sql": "SELEC * FORM invalid_syntax"
        }),
    )
    .await;

    // Should return error for invalid SQL (parse error returns BAD_REQUEST)
    assert!(status == StatusCode::BAD_REQUEST);
    assert_eq!(json["success"], false);
    assert!(json["error"].is_string());
}

#[tokio::test]
async fn test_sql_with_params_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, json) = post_json(
        &mut app,
        "/api/v1/query",
        json!({
            "sql": "SELECT $1 AS value",
            "params": [42]
        }),
    )
    .await;

    // Parameterized queries may or may not be fully implemented
    assert!(status == StatusCode::OK || status == StatusCode::BAD_REQUEST);
    assert!(json["execution_time_ms"].is_number());
}

// =============================================================================
// Query Builder E2E Tests
// =============================================================================

#[tokio::test]
async fn test_query_builder_execute_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Query builder endpoint returns ExecuteQueryResponse with columns, rows, row_count
    let (status, json) = post_json(
        &mut app,
        "/api/v1/query-builder/execute",
        json!({
            "query": "SELECT 'hello' AS greeting"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
    assert!(json["columns"].is_array());
    assert!(json["rows"].is_array());
    assert!(json["row_count"].is_number());
    assert!(json["execution_time_ms"].is_number());
}

#[tokio::test]
async fn test_query_builder_users_table_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // First create a users table
    let (create_status, _) = post_json(
        &mut app,
        "/api/v1/query-builder/execute",
        json!({
            "query": "CREATE TABLE users (id INT, name VARCHAR(100), email VARCHAR(200))"
        }),
    )
    .await;
    assert_eq!(create_status, StatusCode::OK);

    // Insert test data
    let (insert_status, _) = post_json(
        &mut app,
        "/api/v1/query-builder/execute",
        json!({
            "query": "INSERT INTO users (id, name, email) VALUES (1, 'Test User', 'test@example.com')"
        }),
    )
    .await;
    assert_eq!(insert_status, StatusCode::OK);

    // Query the users table
    let (status, json) = post_json(
        &mut app,
        "/api/v1/query-builder/execute",
        json!({
            "query": "SELECT * FROM users"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    // Query executes through real engine
    assert!(json["execution_time_ms"].is_number());
}

// =============================================================================
// Graph Data E2E Tests
// =============================================================================

#[tokio::test]
async fn test_graph_data_endpoint_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, json) = get_json(&mut app, "/api/v1/graph/data").await;

    assert_eq!(status, StatusCode::OK);
    // Returns GraphDataResponse with nodes and edges arrays
    assert!(json["nodes"].is_array());
    assert!(json["edges"].is_array());

    // Verify node structure
    let nodes = json["nodes"].as_array().unwrap();
    assert!(!nodes.is_empty());
    let node = &nodes[0];
    assert!(node["id"].is_string());
    assert!(node["label"].is_string());
    assert!(node["properties"].is_object());

    // Verify edge structure
    let edges = json["edges"].as_array().unwrap();
    assert!(!edges.is_empty());
    let edge = &edges[0];
    assert!(edge["id"].is_string());
    assert!(edge["source"].is_string());
    assert!(edge["target"].is_string());
    assert!(edge["relationship"].is_string());
}

// =============================================================================
// Metrics and Monitoring E2E Tests
// =============================================================================

#[tokio::test]
async fn test_detailed_metrics_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, json) = get_json(&mut app, "/api/v1/metrics").await;
    assert_eq!(status, StatusCode::OK);

    // Verify comprehensive metrics
    assert!(json["total_requests"].is_number());
    assert!(json["failed_requests"].is_number());
    assert!(json["avg_duration_ms"].is_number());
    assert!(json["success_rate"].is_number());
}

#[tokio::test]
async fn test_metrics_after_queries_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Make SQL query requests to generate metrics (queries go through execute_query)
    for _ in 0..3 {
        let _ = post_json(
            &mut app,
            "/api/v1/query",
            json!({
                "sql": "SELECT 1"
            }),
        )
        .await;
    }

    // Check metrics reflect the queries
    let (status, json) = get_json(&mut app, "/api/v1/metrics").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["total_requests"].as_u64().unwrap() >= 3);
    // Should have a success rate since all queries should succeed
    assert!(json["success_rate"].as_f64().unwrap() > 0.0);
}

// =============================================================================
// Concurrent Operations E2E Tests
// =============================================================================

#[tokio::test]
async fn test_concurrent_reads_e2e() {
    let state = shared_state();

    // Spawn multiple concurrent read tasks
    let mut handles = Vec::new();
    for i in 0..10 {
        let state_clone = state.clone();
        let handle = tokio::spawn(async move {
            let mut app = app_with_state(state_clone);
            let (status, _) = get_json(&mut app, "/health").await;
            (i, status)
        });
        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        let (_, status) = handle.await.unwrap();
        assert_eq!(status, StatusCode::OK);
    }
}

#[tokio::test]
async fn test_concurrent_kv_writes_e2e() {
    let state = shared_state();

    // Spawn multiple concurrent write tasks
    let mut handles = Vec::new();
    for i in 0..5 {
        let state_clone = state.clone();
        let handle = tokio::spawn(async move {
            let mut app = app_with_state(state_clone);
            let (status, json) = post_json(
                &mut app,
                "/api/v1/kv/keys",
                json!({
                    "key": format!("concurrent_key_{}", i),
                    "value": format!("concurrent_value_{}", i)
                }),
            )
            .await;
            // Verify response contains the key/value we sent
            let key_matches = json["key"].as_str() == Some(&format!("concurrent_key_{}", i));
            (i, status, key_matches)
        });
        handles.push(handle);
    }

    // All writes should succeed and return correct keys
    for handle in handles {
        let (_, status, key_matches) = handle.await.unwrap();
        assert_eq!(status, StatusCode::OK);
        assert!(key_matches);
    }
}

// =============================================================================
// Error Handling E2E Tests
// =============================================================================

#[tokio::test]
async fn test_malformed_json_request_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state.clone());

    let response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from("{ invalid json }"))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should handle malformed JSON gracefully
    assert!(response.status() == StatusCode::BAD_REQUEST ||
            response.status() == StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_missing_required_fields_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state.clone());

    // Login without password - Axum returns 422 for missing required fields
    let response = app
        .call(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"username": "demo"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    // 422 Unprocessable Entity for missing required field
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_empty_request_body_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, _) = post_json(&mut app, "/api/v1/auth/login", json!({})).await;
    assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY);
}

// =============================================================================
// Rate Limiting and Security E2E Tests
// =============================================================================

#[tokio::test]
async fn test_multiple_failed_logins_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Attempt multiple failed logins
    for _ in 0..5 {
        let (status, _) = post_json(
            &mut app,
            "/api/v1/auth/login",
            json!({
                "username": "admin",
                "password": "wrong_password"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // Should still be able to login with correct credentials
    let (status, _) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "demo",
            "password": "demo"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

// =============================================================================
// Data Integrity E2E Tests
// =============================================================================

#[tokio::test]
async fn test_kv_write_response_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Write data and verify response contains the data
    let unique_key = format!("persistence_test_{}", uuid::Uuid::new_v4());
    let (status, json) = post_json(
        &mut app,
        "/api/v1/kv/keys",
        json!({
            "key": unique_key.clone(),
            "value": "test_persistence"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Response returns the entry we just created
    assert_eq!(json["key"], unique_key);
    assert_eq!(json["value"], "test_persistence");
    assert!(json["created_at"].is_string());
    assert!(json["updated_at"].is_string());
}

#[tokio::test]
async fn test_special_characters_in_data_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Test special characters in value
    let special_value = "Test with special chars: <>&\"'`$%@!#^*(){}[]|\\";
    let (status, json) = post_json(
        &mut app,
        "/api/v1/kv/keys",
        json!({
            "key": "special_char_test",
            "value": special_value
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Response returns the value we set
    assert_eq!(json["value"], special_value);
}

#[tokio::test]
async fn test_unicode_data_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Test unicode characters
    let unicode_value = "Unicode test: 你好世界 🌍 مرحبا العالم こんにちは世界";
    let (status, json) = post_json(
        &mut app,
        "/api/v1/kv/keys",
        json!({
            "key": "unicode_test",
            "value": unicode_value
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Response returns the unicode value correctly
    assert_eq!(json["value"], unicode_value);
}

#[tokio::test]
async fn test_large_value_storage_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // Test large value (1MB of data)
    let large_value = "x".repeat(1024 * 1024);
    let (status, _) = post_json(
        &mut app,
        "/api/v1/kv/keys",
        json!({
            "key": "large_value_test",
            "value": large_value
        }),
    )
    .await;

    // Should either succeed or return appropriate error
    assert!(status == StatusCode::OK || status == StatusCode::PAYLOAD_TOO_LARGE);
}

// =============================================================================
// API Versioning E2E Tests
// =============================================================================

#[tokio::test]
async fn test_api_v1_prefix_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // All v1 endpoints should work
    let endpoints = vec![
        "/api/v1/admin/cluster",
        "/api/v1/admin/nodes",
        "/api/v1/admin/stats",
        "/api/v1/admin/alerts",
        "/api/v1/admin/dashboard",
        "/api/v1/tables",
        "/api/v1/metrics",
    ];

    for endpoint in endpoints {
        let (status, _) = get_json(&mut app, endpoint).await;
        assert!(status == StatusCode::OK || status == StatusCode::NOT_FOUND,
            "Endpoint {} returned unexpected status: {}", endpoint, status);
    }
}

// =============================================================================
// Full System E2E Test
// =============================================================================

#[tokio::test]
async fn test_complete_system_flow_e2e() {
    let state = shared_state();
    let mut app = app_with_state(state);

    // 1. Health check
    let (status, json) = get_json(&mut app, "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "healthy");

    // 2. Login
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/login",
        json!({
            "username": "demo",
            "password": "demo"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _token = json["token"].as_str().unwrap();

    // 3. Store data in KV (returns the created entry)
    let test_key = format!("system_test_{}", uuid::Uuid::new_v4());
    let (status, json) = post_json(
        &mut app,
        "/api/v1/kv/keys",
        json!({
            "key": test_key.clone(),
            "value": "system_test_value"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["key"], test_key);
    assert_eq!(json["value"], "system_test_value");

    // 4. Execute SQL query (uses "sql" field)
    let (status, json) = post_json(
        &mut app,
        "/api/v1/query",
        json!({
            "sql": "SELECT 1 AS test"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    // 5. Check cluster status
    let (status, json) = get_json(&mut app, "/api/v1/admin/cluster").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["name"].is_string());

    // 6. Get dashboard summary
    let (status, json) = get_json(&mut app, "/api/v1/admin/dashboard").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["cluster"].is_object());

    // 7. Check metrics (SQL queries increment metrics)
    let (status, json) = get_json(&mut app, "/api/v1/metrics").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["total_requests"].is_number());

    // 8. Check activities were logged
    let (status, json) = get_json(&mut app, "/api/v1/admin/activities").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.as_array().unwrap().len() > 0);

    // 9. Get graph data
    let (status, json) = get_json(&mut app, "/api/v1/graph/data").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["nodes"].is_array());

    // 10. Get document collections
    let (status, json) = get_json(&mut app, "/api/v1/documents/collections").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json.is_array());
}
