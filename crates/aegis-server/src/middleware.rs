//! Aegis Middleware
//!
//! HTTP middleware for cross-cutting concerns including request ID generation,
//! authentication, rate limiting, and request logging.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use axum::{
    body::Body,
    http::{Request, Response, HeaderValue},
    middleware::Next,
};
use uuid::Uuid;

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
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
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
}
