---
layout: default
title: aegis-monitoring
parent: Crate Documentation
nav_order: 13
description: "Observability — Prometheus metrics, health checks, distributed tracing"
---

# aegis-monitoring

Observability and metrics for Aegis-DB deployments: Prometheus-compatible
metrics, health checks, and distributed tracing.

## Overview

`aegis-monitoring` provides the telemetry surface for running Aegis-DB in
production — metrics scraping, liveness/readiness probes, and trace correlation.

Key features:

- **Prometheus-compatible metrics** — counters, gauges, histograms, summaries
- **Health checks** — liveness and readiness probes
- **Distributed tracing** — W3C Trace Context support
- **Structured logging** with trace correlation

## Modules

| Module | Responsibility |
|--------|----------------|
| `metrics.rs` | Prometheus-compatible metric registry (counters, gauges, histograms, summaries) |
| `health.rs` | Liveness / readiness health checks |
| `tracing.rs` | Distributed tracing (W3C Trace Context) + structured logging |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/metrics` | Prometheus scrape endpoint |
| GET | `/health` | Liveness probe (always reachable, even before bootstrap) |
| GET | `/health/ready` | Readiness probe |

## Tests

808 tests (workspace total) covering metric registration/scrape formatting,
health probe states, and trace-context propagation.
