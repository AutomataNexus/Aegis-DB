<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/assets/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-monitoring

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.3.1-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Monitoring and observability for the Aegis Database Platform.

## Overview

`aegis-monitoring` provides observability including metrics collection, distributed tracing, and health checks. It exports metrics in Prometheus format.

## Features

- **Metrics Collection** - Counters, gauges, histograms, summaries with label support
- **Distributed Tracing** - Spans, trace context, W3C Trace Context format
- **Health Checks** - Liveness and readiness probes (memory, disk, connection pool, latency)
- **Structured Logging** - Log entries with trace correlation and level filtering
- **Prometheus Export** - Native Prometheus text format export

## Architecture

```
┌─────────────────────────────────────────────────┐
│            Monitoring System                     │
├─────────────────────────────────────────────────┤
│              Metrics Registry                    │
│  ┌──────────┬──────────────┬─────────────────┐  │
│  │ Counters │   Gauges     │  Histograms     │  │
│  │          │              │  Summaries      │  │
│  └──────────┴──────────────┴─────────────────┘  │
├─────────────────────────────────────────────────┤
│              Tracing System                      │
│  ┌──────────┬──────────────┬─────────────────┐  │
│  │  Spans   │   Trace      │  Structured     │  │
│  │          │   Context    │  Logging        │  │
│  └──────────┴──────────────┴─────────────────┘  │
├─────────────────────────────────────────────────┤
│              Health Manager                      │
│          (Liveness / Readiness)                  │
└─────────────────────────────────────────────────┘
```

## Modules

| Module | Description |
|--------|-------------|
| `metrics` | Counters, gauges, histograms, summaries, Prometheus export |
| `tracing` | Spans, trace context, structured logging |
| `health` | Health checks with built-in memory, disk, connection, latency checks |

## Usage

```toml
[dependencies]
aegis-monitoring = { path = "../aegis-monitoring" }
```

### Metrics

```rust
use aegis_monitoring::metrics::{MetricRegistry, DatabaseMetrics};

let registry = MetricRegistry::new();

// Counter - monotonically increasing
let requests = registry.counter("aegis_requests_total", "Total requests");
requests.inc();
requests.inc_by(5);

// Counter with labels
let requests = registry.counter_with_labels(
    "aegis_http_total",
    "Total HTTP requests",
    &[("method", "GET"), ("endpoint", "/api/users")],
);
requests.inc();

// Gauge - can go up or down
let connections = registry.gauge("aegis_connections", "Active connections");
connections.set(42.0);
connections.inc();
connections.dec();

// Histogram - distribution of values
let latency = registry.histogram(
    "aegis_request_duration_seconds",
    "Request latency",
    vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0],
);
latency.observe(0.023);

// Summary - quantile estimation
let summary = registry.summary(
    "aegis_query_seconds",
    "Query duration summary",
);
summary.observe(0.15);

// Pre-built database metrics
let db_metrics = DatabaseMetrics::new(&registry);
db_metrics.record_query(0.023, true);
db_metrics.record_read(4096);
```

### Tracing

```rust
use aegis_monitoring::tracing::{Tracer, TraceContext};

let tracer = Tracer::new("aegis-server");

// Create a span
let span_id = tracer.start_span("handle_request");
// ... do work ...
tracer.end_span(span_id);

// Trace context (W3C format)
let ctx = TraceContext::new();
let traceparent = ctx.to_traceparent();
// e.g. "00-<trace_id>-<span_id>-01"

// Structured logging
use aegis_monitoring::tracing::Logger;
let logger = Logger::new("aegis-server");
logger.info("Request handled", &[("status", "200"), ("path", "/api/users")]);
```

### Health Checks

```rust
use aegis_monitoring::health::{HealthChecker, HealthStatus, MemoryHealthCheck, DiskHealthCheck};

let mut checker = HealthChecker::new();

// Add built-in checks
checker.add_check(Box::new(MemoryHealthCheck::new(80.0, 95.0)));
checker.add_check(Box::new(DiskHealthCheck::new("/", 80.0, 95.0)));

// Run all checks
checker.run_checks();
let report = checker.get_report();
println!("Status: {:?}", report.status);
println!("{}", report.to_json());

// Kubernetes-style probes
use aegis_monitoring::health::ProbeChecker;
let probes = ProbeChecker::new();
let liveness = probes.check_liveness();   // is the process alive?
let readiness = probes.check_readiness(); // is it ready to serve?
```

### Pre-built Metrics

The `DatabaseMetrics` struct provides these pre-registered metrics:

| Metric | Type | Description |
|--------|------|-------------|
| `aegis_queries_total` | Counter | Total queries executed |
| `aegis_query_duration_seconds` | Histogram | Query latency |
| `aegis_active_connections` | Gauge | Active connections |
| `aegis_bytes_read` | Counter | Bytes read from storage |
| `aegis_bytes_written` | Counter | Bytes written to storage |
| `aegis_cache_hits` | Counter | Cache hits |
| `aegis_cache_misses` | Counter | Cache misses |
| `aegis_transactions_total` | Counter | Total transactions |
| `aegis_transaction_duration_seconds` | Histogram | Transaction latency |
| `aegis_errors_total` | Counter | Total errors |

### Prometheus Export

```rust
// Get metrics in Prometheus format
let output = registry.export_prometheus();
// Returns text like:
// # HELP aegis_queries_total Total queries executed
// # TYPE aegis_queries_total counter
// aegis_queries_total 1234
```

## Tests

```bash
cargo test -p aegis-monitoring
```

## License

Apache-2.0
