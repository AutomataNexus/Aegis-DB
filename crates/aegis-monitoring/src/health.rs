//! Aegis Health Checks
//!
//! Health monitoring and status reporting for database components.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

// =============================================================================
// Health Status
// =============================================================================

/// Health status of a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Component is healthy.
    Healthy,
    /// Component is degraded but operational.
    Degraded,
    /// Component is unhealthy.
    Unhealthy,
    /// Component status is unknown.
    Unknown,
}

impl HealthStatus {
    /// Check if the status is healthy.
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Check if the status is operational (healthy or degraded).
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }

    /// Combine two statuses (worst wins).
    pub fn combine(&self, other: &HealthStatus) -> HealthStatus {
        match (self, other) {
            (Self::Unhealthy, _) | (_, Self::Unhealthy) => Self::Unhealthy,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Degraded, _) | (_, Self::Degraded) => Self::Degraded,
            _ => Self::Healthy,
        }
    }
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

// =============================================================================
// Health Check Result
// =============================================================================

/// Result of a health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub duration_ms: u64,
    pub timestamp: u64,
    pub details: HashMap<String, String>,
}

impl HealthCheckResult {
    /// Create a healthy result.
    pub fn healthy(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Healthy,
            message: None,
            duration_ms: 0,
            timestamp: Self::now(),
            details: HashMap::new(),
        }
    }

    /// Create a degraded result.
    pub fn degraded(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Degraded,
            message: Some(message.to_string()),
            duration_ms: 0,
            timestamp: Self::now(),
            details: HashMap::new(),
        }
    }

    /// Create an unhealthy result.
    pub fn unhealthy(name: &str, message: &str) -> Self {
        Self {
            name: name.to_string(),
            status: HealthStatus::Unhealthy,
            message: Some(message.to_string()),
            duration_ms: 0,
            timestamp: Self::now(),
            details: HashMap::new(),
        }
    }

    /// Add a detail.
    pub fn with_detail(mut self, key: &str, value: &str) -> Self {
        self.details.insert(key.to_string(), value.to_string());
        self
    }

    /// Set the duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration_ms = duration.as_millis() as u64;
        self
    }

    /// Get current timestamp.
    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

// =============================================================================
// Health Check Trait
// =============================================================================

/// Trait for implementing health checks.
pub trait HealthCheck: Send + Sync {
    /// Get the name of this health check.
    fn name(&self) -> &str;

    /// Run the health check.
    fn check(&self) -> HealthCheckResult;

    /// Get the check interval.
    fn interval(&self) -> Duration {
        Duration::from_secs(30)
    }

    /// Check if this is a critical health check.
    fn is_critical(&self) -> bool {
        true
    }
}

// =============================================================================
// Built-in Health Checks
// =============================================================================

/// Health check for memory usage.
pub struct MemoryHealthCheck {
    max_usage_percent: f64,
}

impl MemoryHealthCheck {
    /// Create a new memory health check.
    pub fn new(max_usage_percent: f64) -> Self {
        Self { max_usage_percent }
    }
}

impl Default for MemoryHealthCheck {
    fn default() -> Self {
        Self::new(90.0)
    }
}

impl HealthCheck for MemoryHealthCheck {
    fn name(&self) -> &str {
        "memory"
    }

    fn check(&self) -> HealthCheckResult {
        // Simulated memory check (in real impl, would check actual memory)
        let usage = 50.0; // Simulated 50% usage

        if usage >= self.max_usage_percent {
            HealthCheckResult::unhealthy(self.name(), &format!("Memory usage at {}%", usage))
        } else if usage >= self.max_usage_percent * 0.8 {
            HealthCheckResult::degraded(self.name(), &format!("Memory usage at {}%", usage))
        } else {
            HealthCheckResult::healthy(self.name())
                .with_detail("usage_percent", &format!("{:.1}", usage))
        }
    }
}

/// Health check for disk space.
pub struct DiskHealthCheck {
    path: String,
    min_free_bytes: u64,
}

impl DiskHealthCheck {
    /// Create a new disk health check.
    pub fn new(path: &str, min_free_bytes: u64) -> Self {
        Self {
            path: path.to_string(),
            min_free_bytes,
        }
    }
}

impl HealthCheck for DiskHealthCheck {
    fn name(&self) -> &str {
        "disk"
    }

    fn check(&self) -> HealthCheckResult {
        // Simulated disk check
        let free_bytes = 10_000_000_000u64; // 10GB simulated

        if free_bytes < self.min_free_bytes {
            HealthCheckResult::unhealthy(
                self.name(),
                &format!(
                    "Disk space low: {} bytes free on {}",
                    free_bytes, self.path
                ),
            )
        } else if free_bytes < self.min_free_bytes * 2 {
            HealthCheckResult::degraded(
                self.name(),
                &format!("Disk space warning: {} bytes free", free_bytes),
            )
        } else {
            HealthCheckResult::healthy(self.name())
                .with_detail("free_bytes", &free_bytes.to_string())
                .with_detail("path", &self.path)
        }
    }
}

/// Health check for connection pools.
pub struct ConnectionPoolHealthCheck {
    pool_name: String,
    max_connections: usize,
    get_active: Arc<dyn Fn() -> usize + Send + Sync>,
}

impl ConnectionPoolHealthCheck {
    /// Create a new connection pool health check.
    pub fn new<F>(pool_name: &str, max_connections: usize, get_active: F) -> Self
    where
        F: Fn() -> usize + Send + Sync + 'static,
    {
        Self {
            pool_name: pool_name.to_string(),
            max_connections,
            get_active: Arc::new(get_active),
        }
    }
}

impl HealthCheck for ConnectionPoolHealthCheck {
    fn name(&self) -> &str {
        "connection_pool"
    }

    fn check(&self) -> HealthCheckResult {
        let active = (self.get_active)();
        let usage = (active as f64 / self.max_connections as f64) * 100.0;

        if usage >= 95.0 {
            HealthCheckResult::unhealthy(
                self.name(),
                &format!("Connection pool {} exhausted: {}/{}", self.pool_name, active, self.max_connections),
            )
        } else if usage >= 80.0 {
            HealthCheckResult::degraded(
                self.name(),
                &format!("Connection pool {} high usage: {:.1}%", self.pool_name, usage),
            )
        } else {
            HealthCheckResult::healthy(self.name())
                .with_detail("pool", &self.pool_name)
                .with_detail("active", &active.to_string())
                .with_detail("max", &self.max_connections.to_string())
        }
    }
}

/// Health check for latency.
pub struct LatencyHealthCheck {
    name: String,
    max_latency_ms: u64,
    check_fn: Arc<dyn Fn() -> Duration + Send + Sync>,
}

impl LatencyHealthCheck {
    /// Create a new latency health check.
    pub fn new<F>(name: &str, max_latency_ms: u64, check_fn: F) -> Self
    where
        F: Fn() -> Duration + Send + Sync + 'static,
    {
        Self {
            name: name.to_string(),
            max_latency_ms,
            check_fn: Arc::new(check_fn),
        }
    }
}

impl HealthCheck for LatencyHealthCheck {
    fn name(&self) -> &str {
        &self.name
    }

    fn check(&self) -> HealthCheckResult {
        let start = Instant::now();
        let latency = (self.check_fn)();
        let check_duration = start.elapsed();

        let latency_ms = latency.as_millis() as u64;

        if latency_ms > self.max_latency_ms {
            HealthCheckResult::unhealthy(
                &self.name,
                &format!("Latency {}ms exceeds threshold {}ms", latency_ms, self.max_latency_ms),
            )
            .with_duration(check_duration)
        } else if latency_ms > self.max_latency_ms / 2 {
            HealthCheckResult::degraded(
                &self.name,
                &format!("Latency {}ms approaching threshold", latency_ms),
            )
            .with_duration(check_duration)
        } else {
            HealthCheckResult::healthy(&self.name)
                .with_detail("latency_ms", &latency_ms.to_string())
                .with_duration(check_duration)
        }
    }
}

// =============================================================================
// Health Checker
// =============================================================================

/// Manages and runs health checks.
pub struct HealthChecker {
    checks: RwLock<Vec<Arc<dyn HealthCheck>>>,
    results: RwLock<HashMap<String, HealthCheckResult>>,
    last_run: RwLock<Option<Instant>>,
}

impl HealthChecker {
    /// Create a new health checker.
    pub fn new() -> Self {
        Self {
            checks: RwLock::new(Vec::new()),
            results: RwLock::new(HashMap::new()),
            last_run: RwLock::new(None),
        }
    }

    /// Add a health check.
    pub fn add_check(&self, check: Arc<dyn HealthCheck>) {
        self.checks.write().unwrap().push(check);
    }

    /// Run all health checks.
    pub fn run_checks(&self) -> Vec<HealthCheckResult> {
        let checks = self.checks.read().unwrap();
        let mut results = Vec::with_capacity(checks.len());

        for check in checks.iter() {
            let start = Instant::now();
            let mut result = check.check();
            result.duration_ms = start.elapsed().as_millis() as u64;
            results.push(result.clone());
            self.results
                .write()
                .unwrap()
                .insert(check.name().to_string(), result);
        }

        *self.last_run.write().unwrap() = Some(Instant::now());
        results
    }

    /// Get the overall health status.
    pub fn overall_status(&self) -> HealthStatus {
        let results = self.results.read().unwrap();
        let checks = self.checks.read().unwrap();

        let mut status = HealthStatus::Healthy;

        for check in checks.iter() {
            if let Some(result) = results.get(check.name()) {
                if check.is_critical() {
                    status = status.combine(&result.status);
                } else if !result.status.is_healthy() {
                    // Non-critical checks only affect if we're still healthy
                    if status == HealthStatus::Healthy {
                        status = HealthStatus::Degraded;
                    }
                }
            } else {
                // No result yet
                status = status.combine(&HealthStatus::Unknown);
            }
        }

        status
    }

    /// Get results for a specific check.
    pub fn get_result(&self, name: &str) -> Option<HealthCheckResult> {
        self.results.read().unwrap().get(name).cloned()
    }

    /// Get all results.
    pub fn get_all_results(&self) -> HashMap<String, HealthCheckResult> {
        self.results.read().unwrap().clone()
    }

    /// Get the comprehensive health report.
    pub fn get_report(&self) -> HealthReport {
        HealthReport {
            status: self.overall_status(),
            checks: self.get_all_results(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Health Report
// =============================================================================

/// Comprehensive health report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: HealthStatus,
    pub checks: HashMap<String, HealthCheckResult>,
    pub timestamp: u64,
}

impl HealthReport {
    /// Convert to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Get the number of healthy checks.
    pub fn healthy_count(&self) -> usize {
        self.checks.values().filter(|r| r.status.is_healthy()).count()
    }

    /// Get the number of unhealthy checks.
    pub fn unhealthy_count(&self) -> usize {
        self.checks
            .values()
            .filter(|r| r.status == HealthStatus::Unhealthy)
            .count()
    }
}

// =============================================================================
// Liveness and Readiness
// =============================================================================

/// Kubernetes-style liveness/readiness probes.
pub struct ProbeChecker {
    liveness_checks: RwLock<Vec<Arc<dyn HealthCheck>>>,
    readiness_checks: RwLock<Vec<Arc<dyn HealthCheck>>>,
}

impl ProbeChecker {
    /// Create a new probe checker.
    pub fn new() -> Self {
        Self {
            liveness_checks: RwLock::new(Vec::new()),
            readiness_checks: RwLock::new(Vec::new()),
        }
    }

    /// Add a liveness check.
    pub fn add_liveness_check(&self, check: Arc<dyn HealthCheck>) {
        self.liveness_checks.write().unwrap().push(check);
    }

    /// Add a readiness check.
    pub fn add_readiness_check(&self, check: Arc<dyn HealthCheck>) {
        self.readiness_checks.write().unwrap().push(check);
    }

    /// Check liveness.
    pub fn check_liveness(&self) -> bool {
        let checks = self.liveness_checks.read().unwrap();
        for check in checks.iter() {
            let result = check.check();
            if result.status == HealthStatus::Unhealthy {
                return false;
            }
        }
        true
    }

    /// Check readiness.
    pub fn check_readiness(&self) -> bool {
        let checks = self.readiness_checks.read().unwrap();
        for check in checks.iter() {
            let result = check.check();
            if !result.status.is_operational() {
                return false;
            }
        }
        true
    }
}

impl Default for ProbeChecker {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct SimpleCheck {
        status: HealthStatus,
    }

    impl HealthCheck for SimpleCheck {
        fn name(&self) -> &str {
            "simple"
        }

        fn check(&self) -> HealthCheckResult {
            match self.status {
                HealthStatus::Healthy => HealthCheckResult::healthy(self.name()),
                HealthStatus::Degraded => HealthCheckResult::degraded(self.name(), "test"),
                HealthStatus::Unhealthy => HealthCheckResult::unhealthy(self.name(), "test"),
                HealthStatus::Unknown => HealthCheckResult::healthy(self.name()),
            }
        }
    }

    #[test]
    fn test_health_status_combine() {
        assert_eq!(
            HealthStatus::Healthy.combine(&HealthStatus::Healthy),
            HealthStatus::Healthy
        );
        assert_eq!(
            HealthStatus::Healthy.combine(&HealthStatus::Degraded),
            HealthStatus::Degraded
        );
        assert_eq!(
            HealthStatus::Degraded.combine(&HealthStatus::Unhealthy),
            HealthStatus::Unhealthy
        );
    }

    #[test]
    fn test_health_check_result() {
        let result = HealthCheckResult::healthy("test")
            .with_detail("key", "value")
            .with_duration(Duration::from_millis(10));

        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(result.details.get("key"), Some(&"value".to_string()));
        assert_eq!(result.duration_ms, 10);
    }

    #[test]
    fn test_memory_health_check() {
        let check = MemoryHealthCheck::new(90.0);
        let result = check.check();
        assert!(result.status.is_healthy());
    }

    #[test]
    fn test_disk_health_check() {
        let check = DiskHealthCheck::new("/data", 1_000_000_000);
        let result = check.check();
        assert!(result.status.is_healthy());
    }

    #[test]
    fn test_connection_pool_health_check() {
        let check = ConnectionPoolHealthCheck::new("main", 100, || 50);
        let result = check.check();
        assert!(result.status.is_healthy());
    }

    #[test]
    fn test_connection_pool_exhausted() {
        let check = ConnectionPoolHealthCheck::new("main", 100, || 98);
        let result = check.check();
        assert_eq!(result.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_latency_health_check() {
        let check = LatencyHealthCheck::new("db", 100, || Duration::from_millis(10));
        let result = check.check();
        assert!(result.status.is_healthy());
    }

    #[test]
    fn test_health_checker() {
        let checker = HealthChecker::new();

        checker.add_check(Arc::new(SimpleCheck {
            status: HealthStatus::Healthy,
        }));

        let results = checker.run_checks();
        assert_eq!(results.len(), 1);
        assert_eq!(checker.overall_status(), HealthStatus::Healthy);
    }

    #[test]
    fn test_health_checker_degraded() {
        let checker = HealthChecker::new();

        checker.add_check(Arc::new(SimpleCheck {
            status: HealthStatus::Degraded,
        }));

        checker.run_checks();
        assert_eq!(checker.overall_status(), HealthStatus::Degraded);
    }

    #[test]
    fn test_health_report() {
        let checker = HealthChecker::new();
        checker.add_check(Arc::new(SimpleCheck {
            status: HealthStatus::Healthy,
        }));
        checker.run_checks();

        let report = checker.get_report();
        assert_eq!(report.status, HealthStatus::Healthy);
        assert_eq!(report.healthy_count(), 1);
        assert_eq!(report.unhealthy_count(), 0);
    }

    #[test]
    fn test_probe_checker() {
        let probes = ProbeChecker::new();

        probes.add_liveness_check(Arc::new(SimpleCheck {
            status: HealthStatus::Healthy,
        }));
        probes.add_readiness_check(Arc::new(SimpleCheck {
            status: HealthStatus::Healthy,
        }));

        assert!(probes.check_liveness());
        assert!(probes.check_readiness());
    }

    #[test]
    fn test_probe_checker_not_ready() {
        let probes = ProbeChecker::new();

        probes.add_readiness_check(Arc::new(SimpleCheck {
            status: HealthStatus::Unhealthy,
        }));

        assert!(!probes.check_readiness());
    }
}
