---
layout: default
title: Crate Documentation
nav_order: 7
has_children: true
description: "Rust crate architecture and documentation"
---

# Crate Documentation
{: .no_toc }

Aegis-DB is built as a Rust workspace with 12 specialized crates.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      aegis-dashboard (WASM)                      │
│                     aegis-cli (CLI Client)                       │
├─────────────────────────────────────────────────────────────────┤
│                    aegis-server (REST API)                       │
├──────────────┬──────────────┬──────────────┬────────────────────┤
│ aegis-client │  aegis-query │ aegis-document│ aegis-timeseries   │
├──────────────┴──────────────┴──────────────┴────────────────────┤
│                   aegis-streaming, aegis-replication             │
├─────────────────────────────────────────────────────────────────┤
│                    aegis-storage (Backends, WAL, MVCC)           │
├─────────────────────────────────────────────────────────────────┤
│                aegis-memory (Arena, Buffer Pool)                 │
├─────────────────────────────────────────────────────────────────┤
│              aegis-common (Types, Errors, Config)                │
└─────────────────────────────────────────────────────────────────┘
```

## Crate Summary

| Crate | Description | Key Dependencies |
|:------|:------------|:-----------------|
| [aegis-common](aegis-common.md) | Core types, config, error handling | serde, chrono, uuid |
| [aegis-memory](aegis-memory.md) | Arena allocators, buffer pool | crossbeam |
| [aegis-storage](aegis-storage.md) | Pluggable backends, WAL, MVCC | lz4, zstd, tokio |
| [aegis-query](aegis-query.md) | SQL parser, planner, executor | sqlparser |
| [aegis-document](aegis-document.md) | Document store (JSON/BSON) | serde_json |
| [aegis-timeseries](aegis-timeseries.md) | Time series with Gorilla compression | chrono |
| [aegis-streaming](aegis-streaming.md) | Real-time streaming engine | tokio |
| [aegis-replication](aegis-replication.md) | Raft, 2PC, CRDTs, sharding | raft-rs |
| [aegis-monitoring](aegis-monitoring.md) | Metrics and health monitoring | prometheus |
| [aegis-client](aegis-client.md) | Client SDK | reqwest, tokio |
| [aegis-server](aegis-server.md) | REST API server | axum, tower |
| [aegis-cli](aegis-cli.md) | Command-line interface | clap |

## Dependency Graph

```
aegis-server
├── aegis-common
├── aegis-storage
├── aegis-query
├── aegis-document
├── aegis-timeseries
├── aegis-streaming
├── aegis-replication
└── aegis-monitoring

aegis-query
├── aegis-common
├── aegis-storage
└── aegis-memory

aegis-storage
├── aegis-common
└── aegis-memory

aegis-replication
├── aegis-common
└── aegis-storage
```

## Building Individual Crates

```bash
# Build a specific crate
cargo build -p aegis-storage

# Test a specific crate
cargo test -p aegis-storage

# Build with features
cargo build -p aegis-server --features "monitoring,replication"
```

## Feature Flags

| Crate | Feature | Description |
|:------|:--------|:------------|
| aegis-server | `tls` | Native TLS termination |
| aegis-server | `monitoring` | Prometheus metrics |
| aegis-server | `replication` | Cluster replication |
| aegis-storage | `compression` | LZ4/Zstd compression |
| aegis-query | `full` | All SQL features |

## Adding a New Crate

1. Create crate directory:
   ```bash
   mkdir crates/aegis-new
   ```

2. Add to workspace in `Cargo.toml`:
   ```toml
   [workspace]
   members = [
       "crates/aegis-new",
       # ...
   ]
   ```

3. Create `Cargo.toml`:
   ```toml
   [package]
   name = "aegis-new"
   version.workspace = true
   edition.workspace = true
   authors.workspace = true
   license.workspace = true

   [dependencies]
   aegis-common = { path = "../aegis-common" }
   ```
