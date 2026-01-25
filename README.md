<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/assets/main/AegisDB-logo.png" alt="AegisDB Logo" width="400">
</p>

<h1 align="center">Aegis Database Platform</h1>

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/build-passing-brightgreen.svg" alt="Build Status">
  <img src="https://img.shields.io/badge/tests-463%20passing-brightgreen.svg" alt="Tests">
  <img src="https://img.shields.io/badge/version-0.1.7-blue.svg" alt="Version">
  <img src="https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-lightgrey.svg" alt="Platform">
</p>

A unified, multi-paradigm database platform built in Rust. Combines relational, time series, document, and real-time streaming capabilities into a single high-performance system.

## Highlights

- **Multi-Paradigm**: SQL, Time Series, Document, Graph, and Streaming in one platform
- **Distributed**: Raft consensus, sharding, and multi-region replication
- **High Performance**: Vectorized execution, Gorilla compression, zero-copy serialization
- **Production Ready**: RBAC, audit logging, TLS, OAuth2/LDAP authentication
- **Modern Stack**: Rust backend, Leptos/WASM dashboard, REST API

## Features

### Multi-Paradigm Data Models
- **Relational/SQL** - Full SQL parser, analyzer, planner, and executor
- **Time Series** - Gorilla compression, aggregations, retention policies
- **Document Store** - JSON storage, full-text search, schema validation
- **Real-time Streaming** - Pub/sub, CDC, event sourcing

### Distributed Systems
- Raft consensus for leader election and replication
- Consistent hashing (HashRing, JumpHash, Rendezvous)
- Distributed transactions with 2PC
- CRDTs for conflict-free replication
- Vector clocks for causality tracking

### Storage Engine
- Pluggable storage backends (Memory, Local filesystem)
- Block compression (LZ4, Zstd, Snappy)
- MVCC with snapshot isolation
- Write-ahead logging for durability
- Buffer pool with LRU eviction

### Web Dashboard
- Leptos/WASM frontend with NexusForge theme
- Authentication with MFA support
- Cluster and node monitoring
- Database paradigm browsers (KV, Documents, Graph, Query Builder)
- Real-time activity feed

### Enterprise Security
- LDAP/Active Directory integration
- OAuth2/OIDC authentication
- Role-based access control (RBAC) with 25+ permissions
- Row-level security policies
- Comprehensive audit logging (100k+ entries)
- Compliance reporting and export

### SDKs and Integrations
- **Python SDK** - Async-first with aiohttp, query builder, transactions
- **JavaScript/TypeScript SDK** - Full type definitions, streaming support
- **Grafana Data Source** - SQL, time series, annotations, variables

### API Endpoints
```
Core:           GET  /health, POST /api/v1/query, GET /api/v1/tables
Admin:          GET  /api/v1/admin/{cluster,dashboard,nodes,storage,stats,alerts,activities}
Auth:           POST /api/v1/auth/{login,mfa/verify,logout}, GET /session, /me
Key-Value:      GET/POST /api/v1/kv/keys, DELETE /api/v1/kv/keys/:key
Documents:      GET  /api/v1/documents/collections, /collections/:name
Graph:          GET  /api/v1/graph/data
Query Builder:  POST /api/v1/query-builder/execute
```

## Project Structure

```
aegis-db/
├── crates/
│   ├── aegis-common/          # Shared types and utilities
│   ├── aegis-storage/         # Storage engine (backends, WAL, transactions)
│   ├── aegis-memory/          # Arena allocators, buffer pool
│   ├── aegis-query/           # SQL parser, analyzer, planner, executor
│   ├── aegis-server/          # REST API server (Axum) + Enterprise Auth
│   ├── aegis-client/          # Client SDK with connection pooling
│   ├── aegis-cli/             # Command-line interface
│   ├── aegis-replication/     # Raft, sharding, 2PC, CRDTs
│   ├── aegis-timeseries/      # Time series engine
│   ├── aegis-document/        # Document store
│   ├── aegis-streaming/       # Real-time streaming
│   ├── aegis-monitoring/      # Metrics, tracing, health checks
│   └── aegis-dashboard/       # Web UI (Leptos/WASM)
├── deploy/
│   └── helm/aegis-db/         # Kubernetes Helm charts
├── integrations/
│   └── grafana-datasource/    # Grafana data source plugin
├── sdks/
│   ├── python/                # Python SDK (aegis-db)
│   └── javascript/            # JavaScript/TypeScript SDK (@aegis-db/client)
├── docs/
│   └── AegisQL.md             # Query language reference
├── Aegis_Architecture.md      # Technical architecture
├── LICENSE.md                 # Apache 2.0 License
├── COMMERCIAL.md              # Commercial licensing options
├── TERMS.md                   # Terms of Service
└── PRIVACY.md                 # Privacy Policy
```

## Installation

### Prerequisites

- **Rust** (1.75+): https://rustup.rs/
- **Trunk** (WASM bundler): `cargo install trunk`

### Quick Install

```bash
git clone https://github.com/automatanexus/aegis-db.git
cd aegis-db
./install.sh
```

The installer will:
1. Check for required tools (cargo, trunk)
2. Build the server and dashboard (release mode)
3. Install the `aegis` command to `~/.local/bin`

After installation, you can run `aegis` from anywhere.

### Usage

```bash
aegis start      # Start server + dashboard
aegis stop       # Stop all services
aegis restart    # Restart all services
aegis status     # Check service status
aegis build      # Rebuild everything
aegis logs       # View recent logs
```

**Individual Services:**
```bash
aegis server start|stop|restart      # Server only (port 9090)
aegis dashboard start|stop|restart   # Dashboard only (port 8000)
aegis logs server                    # Follow server logs
aegis logs dashboard                 # Follow dashboard logs
```

### Access

After running `aegis start`:

| Service   | URL                         |
|-----------|----------------------------|
| Dashboard | http://localhost:8000      |
| API       | http://localhost:9090      |
| Health    | http://localhost:9090/health |

**Default Login:** `demo` / `demo`

### Uninstall

```bash
./install.sh uninstall
```

---

## Development

### Build from Source
```bash
cargo build --workspace
```

### Run Tests
```bash
cargo test --workspace
```

### Run Server (dev mode)
```bash
cargo run -p aegis-server
```

### Build Dashboard (requires Trunk)
```bash
cd crates/aegis-dashboard
trunk build --release
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE.md](LICENSE.md) for details.

Copyright 2024-2026 AutomataNexus Development Team

## Documentation

- [Architecture](Aegis_Architecture.md) - Technical design and system overview
- [AegisQL Reference](docs/AegisQL.md) - Complete query language documentation
- [Commercial Licensing](COMMERCIAL.md) - Enterprise features and pricing
- [API Documentation](docs/) - REST API and SDK references

## Community

- **GitHub Issues**: Report bugs and request features
- **Discord**: Join our community chat
- **Stack Overflow**: Tag questions with `aegis-db`

## Contributing

We welcome contributions! Please see our contribution guidelines before submitting PRs.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
