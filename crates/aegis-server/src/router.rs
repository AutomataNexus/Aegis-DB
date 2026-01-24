//! Aegis Router
//!
//! HTTP router configuration with middleware stack. Defines all API routes
//! and applies cross-cutting concerns like logging, CORS, and rate limiting.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::handlers;
use crate::middleware;
use crate::state::AppState;
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

// =============================================================================
// Router
// =============================================================================

/// Create the main application router.
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        .route("/query", post(handlers::execute_query))
        .route("/tables", get(handlers::list_tables))
        .route("/tables/:name", get(handlers::get_table))
        .route("/metrics", get(handlers::get_metrics));

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
        .route("/activities", get(handlers::get_activities));

    let auth_routes = Router::new()
        .route("/login", post(handlers::login))
        .route("/mfa/verify", post(handlers::verify_mfa))
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
        .route("/data", get(handlers::get_graph_data));

    // Query builder routes
    let query_routes = Router::new()
        .route("/execute", post(handlers::execute_builder_query));

    Router::new()
        .route("/health", get(handlers::health_check))
        .nest("/api/v1", api_routes)
        .nest("/api/v1/admin", admin_routes)
        .nest("/api/v1/auth", auth_routes)
        .nest("/api/v1/kv", kv_routes)
        .nest("/api/v1/documents", doc_routes)
        .nest("/api/v1/timeseries", timeseries_routes)
        .nest("/api/v1/streaming", streaming_routes)
        .nest("/api/v1/graph", graph_routes)
        .nest("/api/v1/query-builder", query_routes)
        .fallback(handlers::not_found)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
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
                    .unwrap(),
            )
            .await
            .unwrap();

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
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
