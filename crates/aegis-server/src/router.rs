//! Aegis Router
//!
//! HTTP router configuration with middleware stack. Defines all API routes
//! and applies cross-cutting concerns like logging, CORS, and rate limiting.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::backup;
use crate::breach;
use crate::compress;
use crate::consent;
use crate::gdpr;
use crate::handlers;
use crate::middleware;
use crate::shield_handlers;
use crate::state::AppState;
use crate::vault_handlers;
use axum::http::{header, Method};
use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

// =============================================================================
// Router
// =============================================================================

/// Create the main application router.
pub fn create_router(state: AppState) -> Router {
    // Check for CORS origins from environment variable
    let env_cors_origins: Vec<String> = std::env::var("AEGIS_CORS_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string())
        .collect();

    let cors_origins = if !env_cors_origins.is_empty() {
        env_cors_origins
    } else {
        state.config.cors_allowed_origins.clone()
    };

    // Configure CORS based on allowed origins
    let cors = if cors_origins.is_empty() {
        // No origins configured = allow common local development origins
        CorsLayer::new()
            .allow_origin(AllowOrigin::list([
                format!("http://{}:{}", state.config.host, state.config.port)
                    .parse()
                    .unwrap(),
                "http://localhost:8000".parse().unwrap(),
                "http://127.0.0.1:8000".parse().unwrap(),
                "http://localhost:3000".parse().unwrap(),
            ]))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::PATCH,
            ])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_credentials(true)
    } else if cors_origins.iter().any(|o| o == "*") {
        // Wildcard = allow any origin — credentials DISABLED for security (CSRF prevention)
        tracing::warn!("CORS configured to allow any origin — credentials disabled for security");
        CorsLayer::new()
            .allow_origin(AllowOrigin::any())
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::PATCH,
            ])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_credentials(false)
    } else {
        // Specific origins configured
        let origins: Vec<_> = cors_origins.iter().filter_map(|o| o.parse().ok()).collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::PATCH,
            ])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_credentials(true)
    };

    let api_routes = Router::new()
        .route("/query", post(handlers::execute_query))
        .route("/tables", get(handlers::list_tables))
        .route("/tables/:name", get(handlers::get_table))
        .route("/metrics", get(handlers::get_metrics))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::rate_limit,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ));

    // Admin read routes: any authenticated role may view dashboards/stats.
    let admin_read_routes = Router::new()
        .route("/cluster", get(handlers::get_cluster_info))
        .route("/dashboard", get(handlers::get_dashboard_summary))
        .route("/nodes", get(handlers::get_nodes))
        .route("/nodes/:node_id/logs", get(handlers::get_node_logs))
        .route("/storage", get(handlers::get_storage_info))
        .route("/stats", get(handlers::get_query_stats))
        .route("/database", get(handlers::get_database_stats))
        .route("/alerts", get(handlers::get_alerts))
        .route("/activities", get(handlers::get_activities))
        .route("/settings", get(handlers::get_settings))
        .route("/users", get(handlers::list_users))
        .route("/roles", get(handlers::list_roles))
        .route(
            "/metrics/timeseries",
            post(handlers::get_metrics_timeseries),
        )
        .route("/backups", get(backup::list_backups))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ));

    // Admin write routes: destructive/privileged operations require the Admin role.
    let admin_write_routes = Router::new()
        .route("/nodes/:node_id/restart", post(handlers::restart_node))
        .route("/nodes/:node_id/drain", post(handlers::drain_node))
        .route("/nodes/:node_id", delete(handlers::remove_node))
        .route("/settings", put(handlers::update_settings))
        .route("/users", post(handlers::create_user))
        .route("/users/:username", put(handlers::update_user))
        .route("/users/:username", delete(handlers::delete_user))
        .route("/roles", post(handlers::create_role))
        .route("/roles/:name", delete(handlers::delete_role))
        .route("/backup", post(backup::create_backup))
        .route("/restore", post(backup::restore_backup))
        .route("/backup/:id", delete(backup::delete_backup))
        .route("/compress", post(compress::compress_section))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_admin,
        ));

    let admin_routes = admin_read_routes.merge(admin_write_routes);

    // Cluster peer management routes. Inter-node join/heartbeat and read views
    // keep require_auth; shutting the node down requires the Admin role.
    let cluster_routes = Router::new()
        .route("/info", get(handlers::get_node_info))
        .route("/join", post(handlers::cluster_join))
        .route("/heartbeat", post(handlers::cluster_heartbeat))
        .route("/peers", get(handlers::get_peers))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ))
        .merge(
            Router::new()
                .route("/shutdown", post(handlers::cluster_shutdown))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::require_admin,
                )),
        );

    // Login route with rate limiting to prevent brute force attacks
    let login_routes = Router::new()
        .route("/login", post(handlers::login))
        .route("/mfa/verify", post(handlers::verify_mfa))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::login_rate_limit,
        ));

    // Other auth routes without strict rate limiting
    let auth_routes = Router::new()
        .merge(login_routes)
        .route("/logout", post(handlers::logout))
        .route("/session", get(handlers::validate_session))
        .route("/me", get(handlers::get_current_user));

    // Key-Value store routes (require auth)
    let kv_routes = Router::new()
        .route("/keys", get(handlers::list_keys))
        .route("/keys", post(handlers::set_key))
        .route("/keys/:key", get(handlers::get_key))
        .route("/keys/:key", delete(handlers::delete_key))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ));

    // Document store routes (require auth)
    let doc_routes = Router::new()
        .route("/collections", get(handlers::list_collections))
        .route("/collections", post(handlers::create_collection))
        .route(
            "/collections/:name",
            get(handlers::get_collection_documents),
        )
        .route(
            "/collections/:name/documents",
            get(handlers::list_collection_documents),
        )
        .route(
            "/collections/:name/documents",
            post(handlers::insert_document),
        )
        .route(
            "/collections/:name/documents/:id",
            get(handlers::get_document),
        )
        .route(
            "/collections/:name/documents/:id",
            put(handlers::update_document),
        )
        .route(
            "/collections/:name/documents/:id",
            patch(handlers::patch_document),
        )
        .route(
            "/collections/:name/documents/:id",
            delete(handlers::delete_document),
        )
        .route(
            "/collections/:name/query",
            post(handlers::query_collection_documents),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ));

    // Time series routes (require auth)
    let timeseries_routes = Router::new()
        .route("/metrics", get(handlers::list_metrics))
        .route("/metrics", post(handlers::register_metric))
        .route("/write", post(handlers::write_timeseries))
        .route("/query", post(handlers::query_timeseries))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ));

    // Streaming routes (require auth)
    let streaming_routes = Router::new()
        .route("/channels", get(handlers::list_channels))
        .route("/channels", post(handlers::create_channel))
        .route("/publish", post(handlers::publish_event))
        .route(
            "/channels/:channel/history",
            get(handlers::get_channel_history),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ));

    // Graph database routes (require auth)
    let graph_routes = Router::new()
        .route("/data", get(handlers::get_graph_data))
        .route("/nodes", post(handlers::create_graph_node))
        .route("/nodes/:node_id", delete(handlers::delete_graph_node))
        .route("/edges", post(handlers::create_graph_edge))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ));

    // Query builder routes (require auth)
    let query_routes = Router::new()
        .route("/execute", post(handlers::execute_builder_query))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ));

    // OTA Update routes. Reads are open to any authenticated role; creating or
    // executing an update plan can swap the running binary, so it requires Admin.
    let update_routes = Router::new()
        .route("/version", get(handlers::get_update_version))
        .route("/status/:plan_id", get(handlers::get_update_status))
        .route("/history", get(handlers::list_update_plans))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ))
        .merge(
            Router::new()
                .route("/plan", post(handlers::create_update_plan))
                .route("/execute", post(handlers::execute_update_plan))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::require_admin,
                )),
        );

    // GDPR/CCPA compliance read & consent-capture routes (any authenticated role).
    let compliance_read_routes = Router::new()
        .route("/certificates", get(gdpr::list_deletion_certificates))
        .route(
            "/certificates/:cert_id",
            get(gdpr::get_deletion_certificate),
        )
        .route(
            "/certificates/:cert_id/verify",
            get(gdpr::verify_deletion_certificate),
        )
        .route("/audit/:subject_id", get(gdpr::get_deletion_audit))
        .route("/audit/verify", get(gdpr::verify_audit_integrity))
        // Consent management
        .route("/consent", post(consent::record_consent))
        .route("/consent/stats", get(consent::get_consent_stats))
        .route("/consent/:subject_id", get(consent::get_consent_status))
        .route(
            "/consent/:subject_id/history",
            get(consent::get_consent_history),
        )
        .route(
            "/consent/:subject_id/export",
            get(consent::export_consent_data),
        )
        .route(
            "/consent/:subject_id/check/:purpose",
            get(consent::check_consent_status),
        )
        // CCPA Do Not Sell
        .route("/do-not-sell", get(consent::get_do_not_sell_list))
        // Breach detection and notification (HIPAA/GDPR) — read views
        .route("/breaches", get(breach::list_breaches))
        .route("/breaches/stats", get(breach::get_breach_stats))
        .route("/breaches/:id", get(breach::get_breach))
        .route("/breaches/:id/report", get(breach::get_breach_report))
        .route("/security-events", get(breach::list_security_events))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ));

    // Compliance mutations that erase/export personal data or change breach
    // state require the Admin role.
    let compliance_admin_routes = Router::new()
        // Data deletion (GDPR right to erasure - Article 17)
        .route(
            "/data-subject/:identifier",
            delete(gdpr::delete_data_subject),
        )
        // Data export (GDPR right to data portability - Article 20)
        .route("/export", post(gdpr::export_data_subject))
        .route("/consent/:subject_id", delete(consent::delete_consent_data))
        .route(
            "/consent/:subject_id/:purpose",
            delete(consent::withdraw_consent),
        )
        .route("/breaches/cleanup", post(breach::trigger_cleanup))
        .route(
            "/breaches/:id/acknowledge",
            post(breach::acknowledge_breach),
        )
        .route("/breaches/:id/resolve", post(breach::resolve_breach))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_admin,
        ));

    let compliance_routes = compliance_read_routes.merge(compliance_admin_routes);

    // Vault routes. Status/audit/transit-key listing are readable by any
    // authenticated role; everything that touches secret material or the seal
    // requires the Admin role.
    let vault_routes = Router::new()
        .route("/status", get(vault_handlers::vault_status))
        .route("/transit/keys", get(vault_handlers::list_transit_keys))
        .route("/audit", get(vault_handlers::vault_audit))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ))
        .merge(
            Router::new()
                .route("/seal", post(vault_handlers::vault_seal))
                .route("/unseal", post(vault_handlers::vault_unseal))
                .route("/secrets", get(vault_handlers::list_secrets))
                .route("/secrets/:key", get(vault_handlers::get_secret))
                .route("/secrets/:key", put(vault_handlers::set_secret))
                .route("/secrets/:key", delete(vault_handlers::delete_secret))
                .route("/transit/encrypt", post(vault_handlers::transit_encrypt))
                .route("/transit/decrypt", post(vault_handlers::transit_decrypt))
                .route("/transit/keys", post(vault_handlers::create_transit_key))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::require_admin,
                )),
        );

    // Shield routes. Read views are open to any authenticated role; changing
    // blocklists, allowlists, or policy requires Admin.
    let shield_routes = Router::new()
        .route("/status", get(shield_handlers::shield_status))
        .route("/stats", get(shield_handlers::shield_stats))
        .route("/events", get(shield_handlers::shield_events))
        .route("/blocked", get(shield_handlers::list_blocked))
        .route("/allowlist", get(shield_handlers::get_allowlist))
        .route("/policy", get(shield_handlers::get_policy))
        .route("/ip/:ip", get(shield_handlers::get_ip_reputation))
        .route("/feed", get(shield_handlers::shield_feed))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_auth,
        ))
        .merge(
            Router::new()
                .route("/blocked", post(shield_handlers::block_ip))
                .route("/blocked/:ip", delete(shield_handlers::unblock_ip))
                .route("/allowlist", post(shield_handlers::add_to_allowlist))
                .route(
                    "/allowlist/:ip",
                    delete(shield_handlers::remove_from_allowlist),
                )
                .route("/policy", put(shield_handlers::update_policy))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::require_admin,
                )),
        );

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
        .nest("/api/v1/updates", update_routes)
        .nest("/api/v1/compliance", compliance_routes)
        .nest("/api/v1/vault", vault_routes)
        .nest("/api/v1/shield", shield_routes)
        .fallback(handlers::not_found)
        .layer(axum::extract::DefaultBodyLimit::max(
            state.config.body_limit_bytes,
        ))
        .layer(tower::limit::ConcurrencyLimitLayer::new(
            state.config.max_connections,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::security_headers,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::shield_check,
        ))
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
