<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/Aegis-DB/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-monitoring

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.0-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Monitoring and observability for the Aegis Database Platform.

## Overview

`aegis-monitoring` provides comprehensive observability including metrics collection, distributed tracing, health checks, and alerting. It integrates with Prometheus, Grafana, and OpenTelemetry.

## Features

- **Metrics Collection** - Counters, gauges, histograms
- **Distributed Tracing** - Request tracing across services
- **Health Checks** - Liveness and readiness probes
- **Alerting** - Configurable alert rules
- **Dashboards** - Pre-built Grafana dashboards

## Architecture

```
┌─────────────────────────────────────────────────┐
│            Monitoring System                     │
├─────────────────────────────────────────────────┤
│              Metrics Collector                   │
│  ┌──────────┬──────────────┬─────────────────┐  │
│  │ Counters │   Gauges     │  Histograms     │  │
│  └──────────┴──────────────┴─────────────────┘  │
├─────────────────────────────────────────────────┤
│              Tracing System                      │
│  ┌──────────┬──────────────┬─────────────────┐  │
│  │  Spans   │   Context    │   Exporters     │  │
│  │          │  Propagation │                 │  │
│  └──────────┴──────────────┴─────────────────┘  │
├─────────────────────────────────────────────────┤
│              Health Manager                      │
│          (Liveness / Readiness)                  │
└─────────────────────────────────────────────────┘
```

## Modules

| Module | Description |
|--------|-------------|
| `metrics` | Metrics collection and export |
| `tracing` | Distributed tracing |
| `health` | Health check system |

## Usage

```toml
[dependencies]
aegis-monitoring = { path = "../aegis-monitoring" }
```

### Metrics

```rust
use aegis_monitoring::metrics::{Counter, Gauge, Histogram, MetricsRegistry};

let registry = MetricsRegistry::new();

// Counter - monotonically increasing
let requests = registry.counter("aegis_requests_total", "Total requests");
requests.inc();
requests.inc_by(5);

// Gauge - can go up or down
let connections = registry.gauge("aegis_connections", "Active connections");
connections.set(42);
connections.inc();
connections.dec();

// Histogram - distribution of values
let latency = registry.histogram(
    "aegis_request_duration_seconds",
    "Request latency",
    vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0],
);
latency.observe(0.023);

// Labels
let requests = registry.counter_vec(
    "aegis_requests_total",
    "Total requests",
    &["method", "endpoint"],
);
requests.with_labels(&["GET", "/api/users"]).inc();
```

### Tracing

```rust
use aegis_monitoring::tracing::{Tracer, Span, SpanContext};

let tracer = Tracer::new("aegis-server");

// Create a span
let span = tracer.start_span("handle_request");
span.set_attribute("http.method", "GET");
span.set_attribute("http.url", "/api/users");

// Child span
let child = span.start_child("database_query");
child.set_attribute("db.statement", "SELECT * FROM users");
// ... do work ...
child.end();

// Add events
span.add_event("cache_miss", vec![("key", "users:123")]);

// End span
span.end();
```

### Context Propagation

```rust
use aegis_monitoring::tracing::{propagate, extract};

// Inject context into headers (for outgoing requests)
let mut headers = HashMap::new();
propagate(&span.context(), &mut headers);

// Extract context from headers (for incoming requests)
let context = extract(&request.headers());
let span = tracer.start_span_with_context("child_operation", context);
```

### Health Checks

```rust
use aegis_monitoring::health::{HealthChecker, HealthStatus, Check};

let health = HealthChecker::new();

// Add checks
health.add_check("database", Check::new(|| async {
    if database.ping().await.is_ok() {
        HealthStatus::Healthy
    } else {
        HealthStatus::Unhealthy("Connection failed".into())
    }
}));

health.add_check("disk_space", Check::new(|| async {
    let usage = get_disk_usage()?;
    if usage < 90.0 {
        HealthStatus::Healthy
    } else if usage < 95.0 {
        HealthStatus::Degraded(format!("Disk {}% full", usage))
    } else {
        HealthStatus::Unhealthy(format!("Disk {}% full", usage))
    }
}));

// Check health
let status = health.check_all().await;
println!("Overall: {:?}", status.overall);
for (name, result) in status.checks {
    println!("  {}: {:?}", name, result);
}

// HTTP endpoints
// GET /health/live   -> Liveness probe
// GET /health/ready  -> Readiness probe
```

### Pre-built Metrics

The following metrics are automatically collected:

| Metric | Type | Description |
|--------|------|-------------|
| `aegis_requests_total` | Counter | Total HTTP requests |
| `aegis_request_duration_seconds` | Histogram | Request latency |
| `aegis_connections_active` | Gauge | Active connections |
| `aegis_queries_total` | Counter | Total queries executed |
| `aegis_query_duration_seconds` | Histogram | Query latency |
| `aegis_storage_bytes` | Gauge | Storage used |
| `aegis_cache_hits_total` | Counter | Cache hits |
| `aegis_cache_misses_total` | Counter | Cache misses |
| `aegis_replication_lag_seconds` | Gauge | Replication lag |

### Prometheus Export

```rust
// Get metrics in Prometheus format
let output = registry.export_prometheus();
// Returns text like:
// # HELP aegis_requests_total Total requests
// # TYPE aegis_requests_total counter
// aegis_requests_total 1234
```

### Grafana Integration

Pre-built dashboards are available in `/integrations/grafana-datasource/`:

- **Aegis Overview** - Cluster health, throughput, latency
- **Query Performance** - Query metrics and slow query log
- **Storage Metrics** - Disk usage, compaction, WAL
- **Replication Status** - Raft state, lag, throughput

## Configuration

```toml
[monitoring]
enabled = true

[monitoring.metrics]
export_interval = "15s"
prometheus_endpoint = "/metrics"

[monitoring.tracing]
enabled = true
sample_rate = 0.1
exporter = "jaeger"
jaeger_endpoint = "http://localhost:14268/api/traces"

[monitoring.health]
check_interval = "10s"
```

## Tests

```bash
cargo test -p aegis-monitoring
```

**Test count:** 35 tests

## License

Apache-2.0
