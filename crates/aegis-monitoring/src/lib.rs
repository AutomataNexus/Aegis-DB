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

pub mod metrics;
pub mod health;
pub mod tracing;

pub use metrics::{
    Counter, Gauge, Histogram, Summary, MetricRegistry, DatabaseMetrics,
    MetricType, MetricValue, HistogramValue, SummaryValue,
};
pub use health::{
    HealthStatus, HealthCheck, HealthCheckResult, HealthChecker, HealthReport,
    ProbeChecker, MemoryHealthCheck, DiskHealthCheck, ConnectionPoolHealthCheck,
    LatencyHealthCheck,
};
pub use tracing::{
    TraceId, SpanId, Span, SpanBuilder, SpanKind, SpanStatus, SpanEvent,
    TraceContext, Tracer, LogLevel, LogEntry, Logger,
};
