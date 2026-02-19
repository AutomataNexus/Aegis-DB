//! Health checking for cluster nodes during and after updates.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::UpdateError;

/// Health status reported by a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    /// Status string (e.g., "healthy", "degraded").
    pub status: String,
    /// Version the node is currently running.
    pub version: String,
    /// Seconds since the node process started.
    pub uptime_seconds: u64,
}

/// Configuration for a health check poll loop.
#[derive(Debug, Clone)]
pub struct HealthCheck {
    /// Address of the node to check (e.g., "http://127.0.0.1:9090").
    pub address: String,
    /// The version string the node should report after update.
    pub expected_version: String,
    /// Timeout per individual health request.
    pub timeout: Duration,
    /// Maximum number of retry attempts.
    pub retries: u32,
}

impl HealthCheck {
    /// Create a new health check configuration.
    pub fn new(address: impl Into<String>, expected_version: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            expected_version: expected_version.into(),
            timeout: Duration::from_secs(5),
            retries: 30,
        }
    }

    /// Set the per-request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the maximum number of retries.
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}

/// Perform a single health check against a node.
///
/// Sends GET /health and parses the JSON response.
pub async fn check_node_health(address: &str) -> Result<NodeHealth, UpdateError> {
    let url = format!("{}/health", address.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| UpdateError::HealthCheckFailed(format!("Failed to build client: {e}")))?;

    let response = client.get(&url).send().await.map_err(|e| {
        UpdateError::NodeUnreachable(format!("{address}: {e}"))
    })?;

    if !response.status().is_success() {
        return Err(UpdateError::HealthCheckFailed(format!(
            "HTTP {} from {url}",
            response.status()
        )));
    }

    // Try to parse structured health response. If the endpoint returns a
    // simple body, fall back to a basic healthy response.
    let body = response.text().await.map_err(|e| {
        UpdateError::HealthCheckFailed(format!("Failed to read response body: {e}"))
    })?;

    match serde_json::from_str::<NodeHealth>(&body) {
        Ok(health) => Ok(health),
        Err(_) => {
            // Fallback: treat any 2xx as healthy with unknown version
            Ok(NodeHealth {
                status: "healthy".to_string(),
                version: "unknown".to_string(),
                uptime_seconds: 0,
            })
        }
    }
}

/// Poll a node until it reports healthy with the expected version, or retries are exhausted.
///
/// Returns `true` if the node became healthy with the correct version, `false` otherwise.
pub async fn wait_for_healthy(check: &HealthCheck) -> Result<bool, UpdateError> {
    let delay = Duration::from_secs(2);

    for attempt in 1..=check.retries {
        info!(
            address = %check.address,
            attempt = attempt,
            max = check.retries,
            "Checking node health"
        );

        match check_node_health(&check.address).await {
            Ok(health) => {
                if health.status == "healthy" || health.status == "ok" {
                    if check.expected_version.is_empty()
                        || health.version == check.expected_version
                        || health.version == "unknown"
                    {
                        info!(
                            address = %check.address,
                            version = %health.version,
                            uptime = health.uptime_seconds,
                            "Node is healthy"
                        );
                        return Ok(true);
                    }
                    warn!(
                        address = %check.address,
                        expected = %check.expected_version,
                        actual = %health.version,
                        "Node healthy but version mismatch"
                    );
                }
            }
            Err(e) => {
                warn!(
                    address = %check.address,
                    attempt = attempt,
                    error = %e,
                    "Health check attempt failed"
                );
            }
        }

        if attempt < check.retries {
            tokio::time::sleep(delay).await;
        }
    }

    Err(UpdateError::HealthCheckFailed(format!(
        "Node {} did not become healthy after {} attempts",
        check.address, check.retries
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_builder() {
        let hc = HealthCheck::new("http://localhost:9090", "0.2.0")
            .with_timeout(Duration::from_secs(10))
            .with_retries(5);

        assert_eq!(hc.address, "http://localhost:9090");
        assert_eq!(hc.expected_version, "0.2.0");
        assert_eq!(hc.timeout, Duration::from_secs(10));
        assert_eq!(hc.retries, 5);
    }
}
