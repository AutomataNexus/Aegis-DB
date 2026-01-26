//! Aegis Router
//!
//! HTTP router configuration with middleware stack. Defines all API routes
//! and applies cross-cutting concerns like logging, CORS, and rate limiting.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::backup;
use crate::breach;
use crate::consent;
use crate::gdpr;
use crate::handlers;
use crate::middleware;
use crate::state::AppState;
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use axum::http::{header, Method};

// =============================================================================
// Router
// =============================================================================

/// Create the main application router.
pub fn create_router(state: AppState) -> Router {
    // Configure CORS based on allowed origins
    let cors = if state.config.cors_allowed_origins.is_empty() {
        // No origins configured = restrictive (only same-origin requests)
        CorsLayer::new()
            .allow_origin(AllowOrigin::exact(
                format!("http://{}:{}", state.config.host, state.config.port)
                    .parse()
                    .unwrap_or_else(|_| "http://localhost:3000".parse().expect("default CORS origin should be valid"))
            ))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_credentials(true)
    } else if state.config.cors_allowed_origins.iter().any(|o| o == "*") {
        // Wildcard = allow any origin (not recommended for production)
        tracing::warn!("CORS configured to allow any origin - not recommended for production");
        CorsLayer::new()
            .allow_origin(AllowOrigin::any())
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    } else {
        // Specific origins configured
        let origins: Vec<_> = state.config.cors_allowed_origins.iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_credentials(true)
    };

    let api_routes = Router::new()
        .route("/query", post(handlers::execute_query))
        .route("/tables", get(handlers::list_tables))
        .route("/tables/:name", get(handlers::get_table))
        .route("/metrics", get(handlers::get_metrics));

    // Admin routes require authentication
    let admin_routes = Router::new()
        .route("/cluster", get(handlers::get_cluster_info))
        .route("/dashboard", get(handlers::get_dashboard_summary))
        .route("/nodes", get(handlers::get_nodes))
        .route("/nodes/:node_id/restart", post(handlers::restart_node))
        .route("/nodes/:node_id/drain", post(handlers::drain_node))
        .route("/nodes/:node_id/logs", get(handlers::get_node_logs))
        .route("/nodes/:node_id", delete(handlers::remove_node))
        .route("/storage", get(handlers::get_storage_info))
        .route("/stats", get(handlers::get_query_stats))
        .route("/database", get(handlers::get_database_stats))
        .route("/alerts", get(handlers::get_alerts))
        .route("/activities", get(handlers::get_activities))
        // Settings management
        .route("/settings", get(handlers::get_settings))
        .route("/settings", put(handlers::update_settings))
        // User management
        .route("/users", get(handlers::list_users))
        .route("/users", post(handlers::create_user))
        .route("/users/:username", put(handlers::update_user))
        .route("/users/:username", delete(handlers::delete_user))
        // Role management
        .route("/roles", get(handlers::list_roles))
        .route("/roles", post(handlers::create_role))
        .route("/roles/:name", delete(handlers::delete_role))
        // Metrics timeseries
        .route("/metrics/timeseries", post(handlers::get_metrics_timeseries))
        // Backup management
        .route("/backup", post(backup::create_backup))
        .route("/backups", get(backup::list_backups))
        .route("/restore", post(backup::restore_backup))
        .route("/backup/:id", delete(backup::delete_backup))
        // Apply authentication middleware to all admin routes
        .layer(axum::middleware::from_fn_with_state(state.clone(), middleware::require_auth));

    // Cluster peer management routes
    let cluster_routes = Router::new()
        .route("/info", get(handlers::get_node_info))
        .route("/join", post(handlers::cluster_join))
        .route("/heartbeat", post(handlers::cluster_heartbeat))
        .route("/peers", get(handlers::get_peers));

    // Login route with rate limiting to prevent brute force attacks
    let login_routes = Router::new()
        .route("/login", post(handlers::login))
        .route("/mfa/verify", post(handlers::verify_mfa))
        .layer(axum::middleware::from_fn_with_state(state.clone(), middleware::login_rate_limit));

    // Other auth routes without strict rate limiting
    let auth_routes = Router::new()
        .merge(login_routes)
        .route("/logout", post(handlers::logout))
        .route("/session", get(handlers::validate_session))
        .route("/me", get(handlers::get_current_user));

    // Key-Value store routes
    let kv_routes = Router::new()
        .route("/keys", get(handlers::list_keys))
        .route("/keys", post(handlers::set_key))
        .route("/keys/:key", get(handlers::get_key))
        .route("/keys/:key", delete(handlers::delete_key));

    // Document store routes
    let doc_routes = Router::new()
        .route("/collections", get(handlers::list_collections))
        .route("/collections", post(handlers::create_collection))
        .route("/collections/:name", get(handlers::get_collection_documents))
        .route("/collections/:name/documents", get(handlers::list_collection_documents))
        .route("/collections/:name/documents", post(handlers::insert_document))
        .route("/collections/:name/documents/:id", get(handlers::get_document))
        .route("/collections/:name/documents/:id", put(handlers::update_document))
        .route("/collections/:name/documents/:id", patch(handlers::patch_document))
        .route("/collections/:name/documents/:id", delete(handlers::delete_document))
        .route("/collections/:name/query", post(handlers::query_collection_documents));

    // Time series routes
    let timeseries_routes = Router::new()
        .route("/metrics", get(handlers::list_metrics))
        .route("/metrics", post(handlers::register_metric))
        .route("/write", post(handlers::write_timeseries))
        .route("/query", post(handlers::query_timeseries));

    // Streaming routes
    let streaming_routes = Router::new()
        .route("/channels", get(handlers::list_channels))
        .route("/channels", post(handlers::create_channel))
        .route("/publish", post(handlers::publish_event))
        .route("/channels/:channel/history", get(handlers::get_channel_history));

    // Graph database routes
    let graph_routes = Router::new()
        .route("/data", get(handlers::get_graph_data))
        .route("/nodes", post(handlers::create_graph_node))
        .route("/nodes/:node_id", delete(handlers::delete_graph_node))
        .route("/edges", post(handlers::create_graph_edge));

    // Query builder routes
    let query_routes = Router::new()
        .route("/execute", post(handlers::execute_builder_query));

    // GDPR/CCPA compliance routes
    let compliance_routes = Router::new()
        // Data deletion (GDPR right to erasure - Article 17)
        .route("/data-subject/:identifier", delete(gdpr::delete_data_subject))
        // Data export (GDPR right to data portability - Article 20)
        .route("/export", post(gdpr::export_data_subject))
        .route("/certificates", get(gdpr::list_deletion_certificates))
        .route("/certificates/:cert_id", get(gdpr::get_deletion_certificate))
        .route("/certificates/:cert_id/verify", get(gdpr::verify_deletion_certificate))
        .route("/audit/:subject_id", get(gdpr::get_deletion_audit))
        .route("/audit/verify", get(gdpr::verify_audit_integrity))
        // Consent management
        .route("/consent", post(consent::record_consent))
        .route("/consent/stats", get(consent::get_consent_stats))
        .route("/consent/:subject_id", get(consent::get_consent_status))
        .route("/consent/:subject_id", delete(consent::delete_consent_data))
        .route("/consent/:subject_id/history", get(consent::get_consent_history))
        .route("/consent/:subject_id/export", get(consent::export_consent_data))
        .route("/consent/:subject_id/check/:purpose", get(consent::check_consent_status))
        .route("/consent/:subject_id/:purpose", delete(consent::withdraw_consent))
        // CCPA Do Not Sell
        .route("/do-not-sell", get(consent::get_do_not_sell_list))
        // Breach detection and notification (HIPAA/GDPR)
        .route("/breaches", get(breach::list_breaches))
        .route("/breaches/stats", get(breach::get_breach_stats))
        .route("/breaches/cleanup", post(breach::trigger_cleanup))
        .route("/breaches/:id", get(breach::get_breach))
        .route("/breaches/:id/acknowledge", post(breach::acknowledge_breach))
        .route("/breaches/:id/resolve", post(breach::resolve_breach))
        .route("/breaches/:id/report", get(breach::get_breach_report))
        .route("/security-events", get(breach::list_security_events))
        // Apply authentication middleware to all compliance routes
        .layer(axum::middleware::from_fn_with_state(state.clone(), middleware::require_auth));

    Router::new()
        .route("/health", get(handlers::health_check))
        .nest("/api/v1", api_routes)
        .nest("/api/v1/admin", admin_routes)
        .nest("/api/v1/cluster", cluster_routes)
        .nest("/api/v1/auth", auth_routes)
        .nest("/api/v1/kv", kv_routes)
        .nest("/api/v1/documents", doc_routes)
        .nest("/api/v1/timeseries", timeseries_routes)
        .nest("/api/v1/streaming", streaming_routes)
        .nest("/api/v1/graph", graph_routes)
        .nest("/api/v1/query-builder", query_routes)
        .nest("/api/v1/compliance", compliance_routes)
        .fallback(handlers::not_found)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(axum::middleware::from_fn_with_state(state.clone(), middleware::security_headers))
        .layer(axum::middleware::from_fn(middleware::request_id))
        .with_state(state)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let state = AppState::new(ServerConfig::default());
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("failed to execute request");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_not_found() {
        let state = AppState::new(ServerConfig::default());
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/nonexistent")
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("failed to execute request");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
