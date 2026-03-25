//! Aegis Monitoring - Observability and Metrics
//!
//! Comprehensive monitoring, metrics collection, health checks, and distributed
//! tracing for Aegis Database deployments.
//!
//! Key Features:
//! - Prometheus-compatible metrics (counters, gauges, histograms, summaries)
//! - Health checks with liveness/readiness probes
//! - Distributed tracing with W3C Trace Context support
//! - Structured logging with trace correlation
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod health;
pub mod metrics;
pub mod tracing;

pub use health::{
    ConnectionPoolHealthCheck, DiskHealthCheck, HealthCheck, HealthCheckResult, HealthChecker,
    HealthReport, HealthStatus, LatencyHealthCheck, MemoryHealthCheck, ProbeChecker,
};
pub use metrics::{
    Counter, DatabaseMetrics, Gauge, Histogram, HistogramValue, MetricRegistry, MetricType,
    MetricValue, Summary, SummaryValue,
};
pub use tracing::{
    LogEntry, LogLevel, Logger, Span, SpanBuilder, SpanEvent, SpanId, SpanKind, SpanStatus,
    TraceContext, TraceId, Tracer,
};
