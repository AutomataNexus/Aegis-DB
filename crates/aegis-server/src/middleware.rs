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
    http::{Request, Response, HeaderValue, StatusCode},
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

        let entry = entries.entry(key.to_string()).or_insert_with(|| RateLimitEntry {
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
pub async fn request_id(
    mut request: Request<Body>,
    next: Next,
) -> Response<Body> {
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
// Authentication Middleware
// =============================================================================

/// Require authentication for protected routes.
/// Returns 401 Unauthorized if no valid session token is provided.
pub async fn require_auth(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response<Body>, impl IntoResponse> {
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
                }))
            ));
        }
    };

    // Validate the session token
    match state.auth.validate_session(token) {
        Some(_user) => {
            // Token is valid, proceed with the request
            Ok(next.run(request).await)
        }
        None => {
            Err((
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Invalid or expired session token",
                    "message": "Please log in again to obtain a new token"
                }))
            ))
        }
    }
}

// =============================================================================
// Rate Limiting Middleware
// =============================================================================

/// Extract client IP from request, checking X-Forwarded-For header first.
fn get_client_ip(request: &Request<Body>) -> String {
    // Check X-Forwarded-For header (from reverse proxies)
    if let Some(forwarded) = request.headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
    {
        // Take the first IP in the chain (original client)
        if let Some(first_ip) = forwarded.split(',').next() {
            return first_ip.trim().to_string();
        }
    }

    // Check X-Real-IP header
    if let Some(real_ip) = request.headers()
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
            }))
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
    if state.login_rate_limiter.check(&format!("login:{}", client_ip)) {
        Ok(next.run(request).await)
    } else {
        Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": "Too many login attempts",
                "message": format!("Too many login attempts. Please try again later. Limit: {} attempts per minute.", rate_limit),
                "retry_after_seconds": 60
            }))
        ))
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;
    use axum::{routing::get, Router};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
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
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("x-request-id"));
    }

    #[tokio::test]
    async fn test_auth_middleware_no_token() {
        let state = AppState::new(ServerConfig::default());

        let app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(state.clone(), require_auth))
            .with_state(state);

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_middleware_invalid_token() {
        let state = AppState::new(ServerConfig::default());

        let app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(state.clone(), require_auth))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("Authorization", "Bearer invalid_token")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_middleware_valid_token() {
        let state = AppState::new(ServerConfig::default());

        // Create a test user and get a valid token
        state.auth.create_user("authtest", "auth@test.com", "TestPassword123!", "admin").unwrap();
        let login_response = state.auth.login("authtest", "TestPassword123!");
        let token = login_response.token.unwrap();

        let app = Router::new()
            .route("/", get(handler))
            .layer(axum::middleware::from_fn_with_state(state.clone(), require_auth))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
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
}
