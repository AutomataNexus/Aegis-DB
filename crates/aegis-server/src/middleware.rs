//! Aegis Middleware
//!
//! HTTP middleware for cross-cutting concerns including request ID generation,
//! authentication, rate limiting, and request logging.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::state::AppState;
use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderValue, Request, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

// =============================================================================
// Rate Limiter
// =============================================================================

/// Token bucket rate limiter entry for a single client.
#[derive(Debug, Clone)]
struct RateLimitEntry {
    tokens: f64,
    last_update: Instant,
}

/// Shared rate limiter state.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    entries: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
    max_requests: u32,
    window_secs: u64,
}

impl RateLimiter {
    /// Create a new rate limiter with the specified requests per minute.
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_requests: requests_per_minute,
            window_secs: 60,
        }
    }

    /// Check if a request from the given key should be allowed.
    /// Returns true if allowed, false if rate limited.
    pub fn check(&self, key: &str) -> bool {
        let mut entries = self.entries.write();
        let now = Instant::now();

        let entry = entries
            .entry(key.to_string())
            .or_insert_with(|| RateLimitEntry {
                tokens: self.max_requests as f64,
                last_update: now,
            });

        // Refill tokens based on elapsed time (token bucket algorithm)
        let elapsed = now.duration_since(entry.last_update);
        let refill_rate = self.max_requests as f64 / self.window_secs as f64;
        let refill = elapsed.as_secs_f64() * refill_rate;
        entry.tokens = (entry.tokens + refill).min(self.max_requests as f64);
        entry.last_update = now;

        // Check if we have tokens available
        if entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Clean up old entries to prevent memory growth.
    pub fn cleanup(&self) {
        let mut entries = self.entries.write();
        let now = Instant::now();
        let max_age = Duration::from_secs(self.window_secs * 2);

        entries.retain(|_, entry| now.duration_since(entry.last_update) < max_age);
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(100) // Default: 100 requests per minute
    }
}

// =============================================================================
// Request ID Middleware
// =============================================================================

/// Add a unique request ID to each request.
pub async fn request_id(mut request: Request<Body>, next: Next) -> Response<Body> {
    let request_id = Uuid::new_v4().to_string();

    request.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&request_id).unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );

    let mut response = next.run(request).await;

    response.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&request_id).unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );

    response
}

// =============================================================================
// Shield Middleware
// =============================================================================

/// Security shield check — runs before all other middleware.
/// Analyzes requests for threats and blocks malicious traffic.
pub async fn shield_check(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, impl IntoResponse> {
    let source_ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("127.0.0.1")
        .split(',')
        .next()
        .unwrap_or("127.0.0.1")
        .trim()
        .to_string();

    let ctx = aegis_shield::RequestContext {
        source_ip: source_ip.clone(),
        path: request.uri().path().to_string(),
        method: request.method().to_string(),
        user_agent: request
            .headers()
            .get("user-agent")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string()),
        auth_user: None,
        body_size: 0,
        headers: std::collections::HashMap::new(),
    };

    match state.shield.analyze_request(&ctx) {
        aegis_shield::ShieldVerdict::Allow => Ok(next.run(request).await),
        aegis_shield::ShieldVerdict::Block {
            reason,
            threat_level,
        } => {
            tracing::warn!(
                ip = %source_ip,
                level = ?threat_level,
                "Shield blocked request: {}",
                reason
            );
            Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "Request blocked by security shield",
                    "reason": reason,
                })),
            ))
        }
        aegis_shield::ShieldVerdict::RateLimit { delay_ms } => {
            // For rate-limited requests, add a delay header but allow through
            let mut response = next.run(request).await;
            if let Ok(val) = HeaderValue::from_str(&delay_ms.to_string()) {
                response.headers_mut().insert("x-ratelimit-delay-ms", val);
            }
            Ok(response)
        }
    }
}

// =============================================================================
// Authentication Middleware
// =============================================================================

/// Handle a request arriving while NO users exist (un-bootstrapped server).
///
/// Fail CLOSED by default: protected endpoints return 503 with provisioning
/// instructions until admin credentials are configured (vault keys or
/// AEGIS_ADMIN_USERNAME/AEGIS_ADMIN_PASSWORD) and the server restarts.
/// `open_bootstrap` (AEGIS_OPEN_BOOTSTRAP=true) restores the legacy fail-open
/// behavior for deployments that relied on it; every such request is logged.
fn no_user_bootstrap_response(
    state: &AppState,
    path: &str,
) -> Option<(StatusCode, Json<serde_json::Value>)> {
    if state.config.open_bootstrap {
        tracing::warn!(
            path = %path,
            "SECURITY: No admin user configured and AEGIS_OPEN_BOOTSTRAP is set — \
             endpoint served WITHOUT authentication. Set AEGIS_ADMIN_USERNAME/\
             AEGIS_ADMIN_PASSWORD (or vault admin credentials) and restart."
        );
        None
    } else {
        tracing::warn!(
            path = %path,
            "Rejected request: no admin user configured (fail-closed bootstrap). \
             Set AEGIS_ADMIN_USERNAME/AEGIS_ADMIN_PASSWORD (or store admin \
             credentials in the vault) and restart to enable access."
        );
        Some((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Server not bootstrapped",
                "message": "No admin user is configured. Provision admin credentials \
                            (AEGIS_ADMIN_USERNAME/AEGIS_ADMIN_PASSWORD environment \
                            variables or vault admin keys) and restart the server."
            })),
        ))
    }
}

/// Require authentication for protected routes.
/// Returns 401 Unauthorized if no valid session token is provided.
pub async fn require_auth(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, impl IntoResponse> {
    // Un-bootstrapped server: fail closed (or fail open if explicitly opted in).
    if state.auth.list_users().is_empty() {
        return match no_user_bootstrap_response(&state, request.uri().path()) {
            Some(rejection) => Err(rejection),
            None => Ok(next.run(request).await),
        };
    }

    // Extract token from Authorization header
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header[7..],
        _ => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Missing or invalid Authorization header",
                    "message": "Provide a valid Bearer token in the Authorization header"
                })),
            ));
        }
    };

    // Validate the session token
    match state.auth.validate_session(token) {
        Some(_user) => {
            // Token is valid, proceed with the request
            Ok(next.run(request).await)
        }
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Invalid or expired session token",
                "message": "Please log in again to obtain a new token"
            })),
        )),
    }
}

/// Require an authenticated user with the **Admin** role.
///
/// Self-contained: validates the session token (like [`require_auth`]) and then
/// enforces `role == Admin`, returning 403 Forbidden otherwise. Apply this to
/// privileged mutation routes (user/role management, node lifecycle, cluster
/// shutdown, vault secrets, OTA updates, backups, shield/GDPR mutations) so a
/// lower-privilege session cannot escalate.
///
/// The no-users bootstrap handling is kept in parity with [`require_auth`]:
/// fail closed by default, legacy fail-open only with AEGIS_OPEN_BOOTSTRAP.
pub async fn require_admin(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, impl IntoResponse> {
    if state.auth.list_users().is_empty() {
        return match no_user_bootstrap_response(&state, request.uri().path()) {
            Some(rejection) => Err(rejection),
            None => Ok(next.run(request).await),
        };
    }

    let token = match request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
    {
        Some(header) if header.starts_with("Bearer ") => header[7..].to_string(),
        _ => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Missing or invalid Authorization header",
                    "message": "Provide a valid Bearer token in the Authorization header"
                })),
            ));
        }
    };

    match state.auth.validate_session(&token) {
        Some(user) if matches!(user.role, crate::auth::UserRole::Admin) => {
            Ok(next.run(request).await)
        }
        Some(user) => {
            tracing::warn!(
                user = %user.username,
                role = %user.role,
                path = %request.uri().path(),
                "Forbidden: admin role required"
            );
            Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "Insufficient privileges",
                    "message": "This operation requires the admin role"
                })),
            ))
        }
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Invalid or expired session token",
                "message": "Please log in again to obtain a new token"
            })),
        )),
    }
}

// =============================================================================
// Rate Limiting Middleware
// =============================================================================

/// Extract client IP from request, checking X-Forwarded-For header first.
fn get_client_ip(request: &Request<Body>) -> String {
    // Check X-Forwarded-For header (from reverse proxies)
    if let Some(forwarded) = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
    {
        // Take the first IP in the chain (original client)
        if let Some(first_ip) = forwarded.split(',').next() {
            return first_ip.trim().to_string();
        }
    }

    // Check X-Real-IP header
    if let Some(real_ip) = request
        .headers()
        .get("x-real-ip")
        .and_then(|h| h.to_str().ok())
    {
        return real_ip.to_string();
    }

    // Fall back to socket address from extensions (if available via ConnectInfo)
    if let Some(connect_info) = request.extensions().get::<ConnectInfo<SocketAddr>>() {
        return connect_info.0.ip().to_string();
    }

    // Ultimate fallback
    "unknown".to_string()
}

/// Rate limiting middleware for general API requests.
/// Returns 429 Too Many Requests if the client exceeds the rate limit.
pub async fn rate_limit(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, impl IntoResponse> {
    let client_ip = get_client_ip(&request);
    let rate_limit = state.config.rate_limit_per_minute;

    // Skip rate limiting if disabled (rate_limit = 0)
    if rate_limit == 0 {
        return Ok(next.run(request).await);
    }

    // Use the rate limiter from AppState
    if state.rate_limiter.check(&client_ip) {
        Ok(next.run(request).await)
    } else {
        Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": "Rate limit exceeded",
                "message": format!("Too many requests. Please try again later. Limit: {} requests per minute.", rate_limit),
                "retry_after_seconds": 60
            })),
        ))
    }
}

/// Rate limiting middleware specifically for login attempts.
/// Uses a stricter limit to prevent brute force attacks.
pub async fn login_rate_limit(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, impl IntoResponse> {
    let client_ip = get_client_ip(&request);
    let rate_limit = state.config.login_rate_limit_per_minute;

    // Skip rate limiting if disabled (rate_limit = 0)
    if rate_limit == 0 {
        return Ok(next.run(request).await);
    }

    // Use the login rate limiter from AppState
    if state
        .login_rate_limiter
        .check(&format!("login:{}", client_ip))
    {
        Ok(next.run(request).await)
    } else {
        Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": "Too many login attempts",
                "message": format!("Too many login attempts. Please try again later. Limit: {} attempts per minute.", rate_limit),
                "retry_after_seconds": 60
            })),
        ))
    }
}

// =============================================================================
// Security Headers Middleware
// =============================================================================

/// Add HTTP security headers to all responses.
/// Includes Content-Security-Policy, X-Content-Type-Options, X-Frame-Options,
/// X-XSS-Protection, Referrer-Policy, and optionally Strict-Transport-Security
/// when TLS is enabled.
pub async fn security_headers(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // Content-Security-Policy: Restrict resource loading to same origin
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'self'"),
    );

    // X-Content-Type-Options: Prevent MIME type sniffing
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );

    // X-Frame-Options: Prevent clickjacking by disabling framing
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));

    // X-XSS-Protection: Enable browser XSS filtering
    headers.insert(
        "x-xss-protection",
        HeaderValue::from_static("1; mode=block"),
    );

    // Referrer-Policy: Control referrer information sent with requests
    headers.insert(
        "referrer-policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // Strict-Transport-Security: Only add when TLS is enabled
    if state.config.tls.is_some() {
        headers.insert(
            "strict-transport-security",
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }

    response
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
    use axum::{routing::get, Router};
    use tower::util::ServiceExt;

    async fn handler() -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn test_request_id_middleware() {
        let app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn(request_id));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("failed to execute request");

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn test_auth_middleware_no_token() {
        let state = AppState::new(ServerConfig::default());
        // Create a user so auth middleware is enforced
        let _ = state
            .auth
            .create_user("testuser", "test@test.local", "TestPass123!", "admin");

        let app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                require_auth,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("failed to execute request");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_middleware_invalid_token() {
        let state = AppState::new(ServerConfig::default());
        // Create a user so auth middleware is enforced
        let _ = state
            .auth
            .create_user("testuser", "test@test.local", "TestPass123!", "admin");

        let app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                require_auth,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("Authorization", "Bearer invalid_token")
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("failed to execute request");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_middleware_valid_token() {
        let state = AppState::new(ServerConfig::default());

        // Create a test user and get a valid token
        state
            .auth
            .create_user("authtest", "auth@test.com", "TestPassword123!", "admin")
            .expect("failed to create test user");
        let login_response = state.auth.login("authtest", "TestPassword123!");
        let token = login_response.token.expect("login should return token");

        let app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                require_auth,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("failed to execute request");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_no_users_fails_closed_by_default() {
        // Force open_bootstrap off so the test is deterministic even if the
        // AEGIS_OPEN_BOOTSTRAP env var is set in this process.
        let config = ServerConfig {
            open_bootstrap: false,
            ..ServerConfig::default()
        };
        let state = AppState::new(config);
        assert!(state.auth.list_users().is_empty());

        let auth_app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                require_auth,
            ))
            .with_state(state.clone());
        let admin_app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                require_admin,
            ))
            .with_state(state.clone());

        for (name, app) in [("require_auth", auth_app), ("require_admin", admin_app)] {
            let response = app
                .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::SERVICE_UNAVAILABLE,
                "{name}: un-bootstrapped server must fail closed"
            );
        }
    }

    #[tokio::test]
    async fn test_no_users_open_bootstrap_optin_allows() {
        let config = ServerConfig {
            open_bootstrap: true,
            ..ServerConfig::default()
        };
        let state = AppState::new(config);
        assert!(state.auth.list_users().is_empty());

        let app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                require_auth,
            ))
            .with_state(state);

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_require_admin_forbids_viewer_allows_admin() {
        let state = AppState::new(ServerConfig::default());
        // An admin must exist so the bootstrap bypass does not apply.
        state
            .auth
            .create_user("rootadmin", "admin@test.local", "AdminPass123!", "admin")
            .expect("create admin");
        state
            .auth
            .create_user("looker", "viewer@test.local", "ViewerPass123!", "viewer")
            .expect("create viewer");

        let app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                require_admin,
            ))
            .with_state(state.clone());

        // Viewer token -> 403 Forbidden
        let viewer_token = state
            .auth
            .login("looker", "ViewerPass123!")
            .token
            .expect("viewer token");
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("Authorization", format!("Bearer {}", viewer_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // Admin token -> 200 OK
        let admin_token = state
            .auth
            .login("rootadmin", "AdminPass123!")
            .token
            .expect("admin token");
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("Authorization", format!("Bearer {}", admin_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn test_rate_limiter_allows_requests() {
        let limiter = RateLimiter::new(10); // 10 requests per minute

        // First 10 requests should be allowed
        for _ in 0..10 {
            assert!(limiter.check("test_client"));
        }

        // 11th request should be rate limited
        assert!(!limiter.check("test_client"));
    }

    #[test]
    fn test_rate_limiter_different_clients() {
        let limiter = RateLimiter::new(5);

        // Each client should have its own limit
        for _ in 0..5 {
            assert!(limiter.check("client_a"));
            assert!(limiter.check("client_b"));
        }

        // Both should now be rate limited
        assert!(!limiter.check("client_a"));
        assert!(!limiter.check("client_b"));
    }

    #[test]
    fn test_rate_limiter_cleanup() {
        let limiter = RateLimiter::new(10);

        // Add some entries
        limiter.check("client_1");
        limiter.check("client_2");

        // Cleanup should not panic
        limiter.cleanup();

        // Should still work after cleanup
        assert!(limiter.check("client_1"));
    }

    #[tokio::test]
    async fn test_security_headers_without_tls() {
        let state = AppState::new(ServerConfig::default());

        let app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                security_headers,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("failed to execute request");

        assert_eq!(response.status(), StatusCode::OK);

        // Check security headers are present
        assert_eq!(
            response
                .headers()
                .get("content-security-policy")
                .map(|v| v.to_str().unwrap()),
            Some("default-src 'self'")
        );
        assert_eq!(
            response
                .headers()
                .get("x-content-type-options")
                .map(|v| v.to_str().unwrap()),
            Some("nosniff")
        );
        assert_eq!(
            response
                .headers()
                .get("x-frame-options")
                .map(|v| v.to_str().unwrap()),
            Some("DENY")
        );
        assert_eq!(
            response
                .headers()
                .get("x-xss-protection")
                .map(|v| v.to_str().unwrap()),
            Some("1; mode=block")
        );
        assert_eq!(
            response
                .headers()
                .get("referrer-policy")
                .map(|v| v.to_str().unwrap()),
            Some("strict-origin-when-cross-origin")
        );

        // HSTS should NOT be present without TLS
        assert!(response
            .headers()
            .get("strict-transport-security")
            .is_none());
    }

    #[tokio::test]
    async fn test_security_headers_with_tls() {
        let config = ServerConfig::default().with_tls("/path/to/cert.pem", "/path/to/key.pem");
        let state = AppState::new(config);

        let app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                security_headers,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("failed to execute request");

        assert_eq!(response.status(), StatusCode::OK);

        // HSTS should be present with TLS
        assert_eq!(
            response
                .headers()
                .get("strict-transport-security")
                .map(|v| v.to_str().unwrap()),
            Some("max-age=31536000; includeSubDomains")
        );
    }
}
