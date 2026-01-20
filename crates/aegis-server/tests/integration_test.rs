//! End-to-end integration tests for Aegis Server
//!
//! Tests the full API flow including authentication, admin endpoints, and activity logging.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::Service;
use serde_json::{json, Value};
use std::sync::Arc;

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
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/mfa/verify",
        json!({
            "code": "123456",
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
    assert!(json["p50_duration_ms"].as_f64().unwrap() < json["p99_duration_ms"].as_f64().unwrap());

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
    let (status, json) = post_json(
        &mut app,
        "/api/v1/auth/mfa/verify",
        json!({
            "code": "123456",
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
