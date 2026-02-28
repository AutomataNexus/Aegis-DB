<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/assets/main/AegisDB-logo.png" alt="AegisDB Logo" width="400">
</p>

<h1 align="center">Aegis-DB</h1>

<p align="center">
  <strong>One database. Every data model. Written in Rust.</strong>
</p>

<p align="center">
  <a href="https://crates.io/crates/aegis-server"><img src="https://img.shields.io/crates/v/aegis-server.svg" alt="crates.io"></a>
  <a href="LICENSE.md"><img src="https://img.shields.io/badge/License-BSL%201.1-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/tests-634%20passing-brightgreen.svg" alt="Tests">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
</p>

<p align="center">
  SQL &bull; Documents &bull; Key-Value &bull; Time Series &bull; Graph &bull; Streaming<br>
  All in a single binary. No external dependencies.
</p>

---

## 30-Second Quickstart

```bash
# Install and run
cargo install aegis-server
aegis-server

# That's it. Server is running on http://localhost:9090
```

```bash
# Create a table
curl -X POST http://localhost:9090/api/v1/query \
  -H "Content-Type: application/json" \
  -d '{"sql": "CREATE TABLE users (id INT, name TEXT, email TEXT)"}'

# Insert data
curl -X POST http://localhost:9090/api/v1/query \
  -H "Content-Type: application/json" \
  -d '{"sql": "INSERT INTO users VALUES (1, '\''Alice'\'', '\''alice@example.com'\'')"}'

# Query it back
curl -X POST http://localhost:9090/api/v1/query \
  -H "Content-Type: application/json" \
  -d '{"sql": "SELECT * FROM users"}'
```

Or install the CLI:

```bash
cargo install aegisdb-cli

aegis-client query "CREATE TABLE users (id INT, name TEXT)"
aegis-client query "INSERT INTO users VALUES (1, 'Alice')"
aegis-client query "SELECT * FROM users"
aegis-client shell  # Interactive SQL shell
```

---

## Why Aegis-DB?

Most projects end up running Postgres + Redis + Elasticsearch + InfluxDB + Kafka. That's 5 databases to deploy, monitor, back up, and keep in sync.

Aegis-DB replaces all of them with a single binary:

| Need | Traditional | Aegis-DB |
|------|-------------|----------|
| Relational data | PostgreSQL | `POST /api/v1/query` with SQL |
| Caching / KV | Redis | `POST /api/v1/kv/keys` |
| Document store | MongoDB | `POST /api/v1/documents/collections` |
| Time series | InfluxDB | `POST /api/v1/timeseries/write` |
| Graph queries | Neo4j | `POST /api/v1/graph/nodes` |
| Event streaming | Kafka | `POST /api/v1/streaming/publish` |

One binary. One port. One backup. One set of credentials.

---

## Benchmarks

Tested on Intel Core Ultra 9 275HX, 55GB RAM, Rust 1.92.0.

### Engine-Level (direct calls, no network)

| Workload | Throughput |
|----------|-----------|
| SQL single-row insert | **223,000 rows/sec** |
| SQL batch insert (1000 rows) | **195,000 rows/sec** |
| KV read (64B values) | **12,350,000 ops/sec** |
| KV write (64B values) | **3,970,000 ops/sec** |
| Fund transfer (indexed UPDATE) | **758,000 TPS** |

### vs SpacetimeDB

| Workload | Aegis-DB | SpacetimeDB | |
|----------|----------|-------------|---|
| Fund transfer, 0% contention | **758,000 TPS** | 107,850 TPS | **7x faster** |
| Fund transfer, high contention | **2,496,000 TPS** | 103,590 TPS | **24x faster** |

### HTTP API (50 concurrent connections)

| Endpoint | Throughput | Avg Latency |
|----------|-----------|-------------|
| SQL Insert | 80,450 ops/sec | 620 μs |
| SQL Read | 40,496 ops/sec | 1.2 ms |
| KV Get | 203,117 ops/sec | 245 μs |

Full results: [benchmarks/RESULTS.md](benchmarks/RESULTS.md)

---

## Features

### Six Data Models, One API

```bash
# SQL
curl -X POST localhost:9090/api/v1/query \
  -d '{"sql": "SELECT * FROM users WHERE age > 21"}'

# Key-Value
curl -X POST localhost:9090/api/v1/kv/keys \
  -d '{"key": "session:abc", "value": {"user_id": 1}}'

# Documents
curl -X POST localhost:9090/api/v1/documents/collections/products/documents \
  -d '{"name": "Widget", "price": 9.99, "tags": ["sale"]}'

# Time Series
curl -X POST localhost:9090/api/v1/timeseries/write \
  -d '{"metric": "cpu_usage", "value": 72.5, "tags": {"host": "web-1"}}'

# Graph
curl -X POST localhost:9090/api/v1/graph/nodes \
  -d '{"label": "Person", "properties": {"name": "Alice"}}'

# Streaming
curl -X POST localhost:9090/api/v1/streaming/publish \
  -d '{"channel": "orders", "event": {"order_id": 123}}'
```

### Multi-Database Isolation

Each application gets its own isolated database:

```json
{"database": "app_one", "sql": "CREATE TABLE users (id INT, name TEXT)"}
{"database": "app_two", "sql": "CREATE TABLE users (id INT, name TEXT)"}
```

Different apps, different schemas, same server. Databases are auto-provisioned on first query.

### Distributed Clustering

```bash
# Start a 3-node cluster
aegis-server --port 9090 --node-name Leader --peers 127.0.0.1:9091,127.0.0.1:9092
aegis-server --port 9091 --node-name Replica1 --peers 127.0.0.1:9090,127.0.0.1:9092
aegis-server --port 9092 --node-name Replica2 --peers 127.0.0.1:9090,127.0.0.1:9091
```

- Raft consensus with leader election
- Consistent hashing for data distribution
- 2-phase commit for distributed transactions
- CRDTs for conflict-free replication
- OTA rolling updates across nodes

### Enterprise Security

- **TLS/HTTPS** with rustls (TLSv1.2/1.3)
- **Argon2id** password hashing
- **RBAC** with 25+ granular permissions
- **OAuth2/OIDC** and **LDAP/Active Directory**
- **MFA** with TOTP (RFC 6238)
- **Rate limiting** (token bucket)
- **HashiCorp Vault** integration
- **Audit logging** (100k+ entries)

### Regulatory Compliance

Built-in support for HIPAA, GDPR, CCPA, SOC 2, and FERPA:

- GDPR right to erasure (Article 17) with deletion certificates
- GDPR data portability (Article 20) with export
- CCPA Do Not Sell tracking
- HIPAA PHI column-level data classification
- Breach detection and notification
- Consent management with full audit trail
- Cryptographic audit log verification

### Storage Engine

- Pluggable backends (Memory, Local filesystem)
- Block compression (LZ4, Zstd, Snappy)
- MVCC with snapshot isolation
- Write-ahead logging
- B-tree and hash indexes
- Buffer pool with LRU eviction

### Web Dashboard

Built-in Leptos/WASM dashboard with:
- Cluster monitoring and node management
- Database browsers for all paradigms
- Query builder and SQL editor
- Real-time activity feed
- User and role management

---

## Installation

### From crates.io

```bash
cargo install aegis-server    # Server
cargo install aegisdb-cli     # CLI client
```

### From source

```bash
git clone https://github.com/AutomataNexus/Aegis-DB.git
cd Aegis-DB
cargo build --release

# Run
./target/release/aegis-server
```

### With persistence

```bash
aegis-server --data-dir /var/lib/aegis/data
```

### With TLS

```bash
# Generate a self-signed cert (or use your own)
openssl req -x509 -newkey rsa:4096 -keyout server.key -out server.crt -days 365 -nodes

aegis-server --tls --tls-cert server.crt --tls-key server.key
```

### Configuration

```bash
export AEGIS_ADMIN_USERNAME=admin
export AEGIS_ADMIN_PASSWORD=your_secure_password

aegis-server --port 9090 --data-dir ./data
```

See [docs/USER_GUIDE.md](docs/USER_GUIDE.md) for full configuration options.

---

## SDKs

### Python

```python
from aegisdb import AegisClient

client = AegisClient("http://localhost:9090")
client.query("CREATE TABLE users (id INT, name TEXT)")
client.query("INSERT INTO users VALUES (1, 'Alice')")
results = client.query("SELECT * FROM users")
```

### JavaScript/TypeScript

```javascript
import { AegisClient } from '@aegis-db/client';

const client = new AegisClient('http://localhost:9090');
await client.query('CREATE TABLE users (id INT, name TEXT)');
await client.query("INSERT INTO users VALUES (1, 'Alice')");
const results = await client.query('SELECT * FROM users');
```

---

## Architecture

```
aegis-server (REST API - Axum)
    |
    ├── aegis-query (SQL parser/planner/executor)
    ├── aegis-document (JSON document store)
    ├── aegis-timeseries (Gorilla compression)
    ├── aegis-streaming (pub/sub, CDC)
    ├── aegis-replication (Raft, sharding, 2PC)
    └── aegis-monitoring (metrics, health)
        |
        ├── aegis-storage (backends, WAL, MVCC)
        ├── aegis-memory (arena allocators, buffer pool)
        └── aegis-common (shared types, errors)
```

13 crates, ~50,000 lines of Rust code, 634 tests.

---

## API Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/api/v1/query` | POST | Execute SQL queries |
| `/api/v1/tables` | GET | List all tables |
| `/api/v1/kv/keys` | GET/POST | List or set key-value pairs |
| `/api/v1/kv/keys/:key` | GET/DELETE | Get or delete a key |
| `/api/v1/documents/collections` | GET/POST | List or create collections |
| `/api/v1/documents/collections/:name/documents` | GET/POST | List or insert documents |
| `/api/v1/timeseries/write` | POST | Write time series data |
| `/api/v1/timeseries/query` | POST | Query time series data |
| `/api/v1/graph/nodes` | POST | Create graph nodes |
| `/api/v1/graph/edges` | POST | Create graph edges |
| `/api/v1/streaming/publish` | POST | Publish events |
| `/api/v1/auth/login` | POST | Authenticate |
| `/api/v1/admin/*` | GET | Admin/monitoring endpoints |
| `/api/v1/import/sql` | POST | Bulk import CSV/JSON data |
| `/api/v1/compliance/*` | GET/POST | GDPR/HIPAA/CCPA endpoints |

Full API docs: [docs/USER_GUIDE.md](docs/USER_GUIDE.md)

---

## Documentation

- [User Guide](docs/USER_GUIDE.md) — Installation, configuration, usage
- [Developer Guide](docs/DEVELOPER_GUIDE.md) — Contributing, architecture deep-dive
- [AegisQL Reference](docs/AegisQL.md) — Query language documentation
- [Security Guide](docs/SECURITY.md) — TLS, Vault, authentication
- [Architecture](Aegis_Architecture.md) — Technical design overview
- [Benchmark Results](benchmarks/RESULTS.md) — Full benchmark data and methodology

---

## License

**Business Source License 1.1** — Free for development, testing, internal use, and non-commercial projects. Commercial database-as-a-service offerings require a license. Converts to Apache 2.0 on January 26, 2030.

See [LICENSE.md](LICENSE.md) for details. Commercial licensing: Devops@automatanexus.com

Copyright 2024-2026 Andrew Jewell Sr / AutomataNexus LLC
