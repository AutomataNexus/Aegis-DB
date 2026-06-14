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
  <img src="https://img.shields.io/badge/tests-808%20passing-brightgreen.svg" alt="Tests">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.85%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/paradigms-8-blueviolet.svg" alt="6 Data Paradigms">
  <img src="https://img.shields.io/badge/LOC-69K%2B-informational.svg" alt="Lines of Code">
</p>

<p align="center">
  SQL &bull; Documents &bull; Key-Value &bull; Time Series &bull; Graph &bull; Streaming &bull; Vector &bull; Full-Text<br>
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
| Vector / KNN | Pinecone / pgvector | `POST /api/v1/vector/collections/:name/search` |
| Full-text search | Elasticsearch | `POST /api/v1/fts/indexes/:name/search` (BM25) |

One binary. One port. One backup. One set of credentials.

---

## Benchmarks

Tested on Intel Core Ultra 9 275HX, 48GB RAM, Rust 1.95.0 (v0.3.1).

### Engine-Level (direct calls, no network)

| Workload | Throughput |
|----------|-----------|
| SQL single-row insert | **204,000 rows/sec** |
| KV read (64B values) | **10,800,000 ops/sec** |
| KV write (64B values) | **4,100,000 ops/sec** |
| KV delete | **27,600,000 ops/sec** |
| Atomic fund transfer (read + verify + debit + credit) | **971,000 TPS** |

### vs SpacetimeDB

The transfer benchmark does the **full transactional work** SpacetimeDB's reducer
does — read both balances, verify funds, then debit + credit atomically under a
single held write lock — a like-for-like comparison (both in-process, in-memory).

| Workload | Aegis-DB | SpacetimeDB | |
|----------|----------|-------------|---|
| Fund transfer, 0% contention | **971,000 TPS** | 107,850 TPS | **9.0x faster** |
| Fund transfer, high contention | **2,542,000 TPS** | 103,590 TPS | **24.5x faster** |

### HTTP API (50 concurrent connections)

| Endpoint | Throughput | Avg Latency |
|----------|-----------|-------------|
| SQL Insert | 72,441 ops/sec | 689 μs |
| SQL Read | 71,587 ops/sec | 697 μs |
| KV Get | 76,523 ops/sec | 652 μs |

Full results: [benchmarks/RESULTS.md](benchmarks/RESULTS.md)

---

## Features

### Eight Data Models, One API

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

# Vector / KNN (HNSW)
curl -X POST localhost:9090/api/v1/vector/collections/docs/search \
  -d '{"vector": [0.12, -0.04, ...], "k": 10, "filter": {"source": "wiki"}}'

# Full-Text Search (BM25)
curl -X POST localhost:9090/api/v1/fts/indexes/articles/search \
  -d '{"query": "rust database", "k": 10}'
```

### Multi-Database Isolation

Each application gets its own isolated database:

```json
{"database": "app_one", "sql": "CREATE TABLE users (id INT, name TEXT)"}
{"database": "app_two", "sql": "CREATE TABLE users (id INT, name TEXT)"}
```

Different apps, different schemas, same server. Databases are auto-provisioned on first query.

### Transactions

```sql
BEGIN;
INSERT INTO accounts VALUES (1, 'Alice', 1000);
INSERT INTO accounts VALUES (2, 'Bob', 500);
COMMIT;
-- Atomic: both rows inserted or neither. ROLLBACK undoes all changes.
```

Multi-statement transactions with snapshot isolation. Auto-rollback on errors.

### Parameterized Queries

```bash
curl -X POST localhost:9090/api/v1/query \
  -d '{"sql": "SELECT * FROM users WHERE id = $1", "params": [42]}'
```

Bind `$1, $2, ...` placeholders to prevent SQL injection and enable plan reuse.

For repeated execution, **prepare once and bind many times** (parsed and planned
once server-side):

```bash
# Returns { "statement_id": "stmt_1" }
curl -X POST localhost:9090/api/v1/prepare -d '{"sql": "SELECT * FROM users WHERE id = $1"}'
curl -X POST localhost:9090/api/v1/prepared/execute -d '{"statement_id": "stmt_1", "params": [42]}'
```

### Distributed Clustering

```bash
# Start a 3-node cluster
aegis-server --port 9090 --node-name Leader --peers 127.0.0.1:9091,127.0.0.1:9092
aegis-server --port 9091 --node-name Replica1 --peers 127.0.0.1:9090,127.0.0.1:9092
aegis-server --port 9092 --node-name Replica2 --peers 127.0.0.1:9090,127.0.0.1:9091
```

- **Mutation replication** — SQL writes forwarded to all peers automatically
- Raft consensus with leader election
- Consistent hashing for data distribution
- 2-phase commit for distributed transactions
- CRDTs for conflict-free replication (8 types)
- OTA rolling updates across nodes

### Aegis-Vault (Integrated Secrets Manager)

Auto-initializes at startup. No external dependencies required.

- **AES-256-GCM encrypted** secret storage on disk
- **Seal/unseal** with passphrase-derived keys (Argon2id)
- **Secret versioning** — keep N previous versions, configurable
- **Transit encryption** — encrypt/decrypt data without storing it
- **Access policies** — control which components see which secrets
- **Audit logging** — every secret access recorded
- **Provider chain** — built-in vault → external HashiCorp Vault → environment variables
- API at `/api/v1/vault/*`

### Aegis-Shield (Security Shield)

Auto-runs as middleware on every request. Zero configuration needed.

- **SQL injection detection** — 30+ regex patterns with scoring (0-100)
- **IP reputation tracking** — per-IP behavior scoring (-100 to +100)
- **Auto-blocking** — configurable thresholds, escalating ban durations
- **Request fingerprinting** — detect scanners (sqlmap, nikto, etc.)
- **Query anomaly detection** — baseline learning, deviation alerting
- **Threat feed** — real-time event stream with statistics
- **Security presets** — Strict / Moderate / Permissive
- **Allowlists** — exempt trusted IPs from all checks
- API at `/api/v1/shield/*`

### Enterprise Security

- **Authentication on all endpoints** — mandatory when admin users configured; startup + per-request security warnings when unconfigured
- **TLS/HTTPS** with rustls (TLSv1.2/1.3)
- **Argon2id** password hashing with session revocation on password change
- **RBAC** with 25+ granular permissions
- **OAuth2/OIDC** and **LDAP/Active Directory**
- **MFA** with TOTP (RFC 6238)
- **Rate limiting** on all data and query endpoints (token bucket)
- **HashiCorp Vault** integration
- **Audit logging** (100k+ entries)
- **Request body limits** (10MB default, configurable)
- **Connection limits** (10K concurrent, configurable)
- **Sanitized error responses** — no internal details leaked to clients
- **CORS security** — wildcard mode disables credentials (CSRF prevention)
- **Constraint enforcement** — NOT NULL, PRIMARY KEY, UNIQUE validated on INSERT
- **Query safety limits** — SELECT results capped at 100K rows

### Regulatory Compliance

Built-in support for HIPAA, GDPR, CCPA, SOC 2, and FERPA:

- GDPR right to erasure (Article 17) with deletion certificates
- GDPR data portability (Article 20) with export
- CCPA Do Not Sell tracking
- HIPAA PHI column-level data classification
- Breach detection and notification
- Consent management with full audit trail
- Cryptographic audit log verification

### SQL Engine

- Full SELECT/INSERT/UPDATE/DELETE/DDL support
- JOINs (INNER, LEFT, RIGHT, FULL, CROSS) with HashJoin and NestedLoop strategies
- **Set operations** — UNION, UNION ALL, INTERSECT, EXCEPT
- GROUP BY, HAVING, ORDER BY, LIMIT/OFFSET, DISTINCT
- Subqueries (IN, EXISTS, scalar)
- 18 scalar functions (UPPER, LOWER, ROUND, CEIL, FLOOR, SUBSTRING, TRIM, NULLIF, NOW, EXTRACT, CONCAT, REPLACE, etc.)
- Parameterized queries ($1, $2) with plan caching
- Table-qualified wildcards (SELECT t.*)

### Document Store

- Full CRUD with schema validation
- 13 filter types (Eq, Ne, Gt, Lt, In, Regex, Contains, etc.)
- **Index-accelerated queries** — hash/btree indexes used for Eq lookups
- Sort, skip, limit, and field projection
- Full-text search with inverted index

### Streaming

- Pub/sub channels with persistent history
- **Live SSE subscription** — `GET /api/v1/streaming/channels/:channel/sse` streams
  events to a client in real time (`text/event-stream`)
- **Consumer groups** with offset tracking and member management
- **Acknowledgment enforcement** (Auto, AtLeastOnce, ExactlyOnce)
- Event filtering, windowed aggregation, CDC

### Storage Engine

- Pluggable backends (Memory, Local filesystem)
- Block compression (LZ4, Zstd, Snappy)
- **MVCC with snapshot isolation** — row-level versioning, readers never block writers
- **Write-ahead logging (WAL)** — crash recovery with automatic replay on startup
- B-tree and hash indexes
- Buffer pool with LRU eviction
- **Query plan cache** — 1024-entry LRU cache, auto-invalidated on DDL changes
- **CDC (Change Data Capture)** — SQL mutations emit events to streaming channels
- **Automatic backups** — scheduled hourly with configurable retention and consistency checkpoints
- **Replication with retry** — mutations forwarded to peers with 3x exponential backoff

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

All data stores (SQL, KV, documents, graph, users, RBAC, settings, consent, breach incidents) are persisted to disk on every mutation and reloaded on startup. WAL provides crash recovery.

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

The Rust, Python, and JavaScript/TypeScript clients all cover the full surface —
SQL (positional `$1` params), prepared statements, KV (+ batch), document CRUD +
cursor-paginated query (+ bulk), time series, graph (+ mutations), schema, health,
metrics, and live SSE channel subscription.

### Python

```python
from aegis_db import AegisClient

async with AegisClient("http://localhost:9090") as client:
    await client.query("INSERT INTO users VALUES ($1, $2)", [1, "Alice"])
    sid = await client.prepare("SELECT * FROM users WHERE id = $1")
    rows = await client.execute_prepared(sid, [1])
    async for event in client.subscribe_channel("orders"):  # live SSE
        ...
```

### JavaScript/TypeScript

```javascript
import { AegisClient } from '@aegis-db/client';

const client = new AegisClient({ url: 'http://localhost:9090' });
await client.connect();
await client.query('INSERT INTO users VALUES ($1, $2)', [1, 'Alice']);
for await (const event of client.subscribeChannel('orders')) { /* live SSE */ }
```

### Rust

```rust
use aegis_client::AegisClient;

let client = AegisClient::connect("aegis://localhost:9090/default").await?;
client.query_with_params("INSERT INTO users VALUES ($1, $2)", vec![1.into(), "Alice".into()]).await?;
let id = client.prepare("SELECT * FROM users WHERE id = $1").await?;
let rows = client.execute_prepared(&id, vec![1.into()]).await?;
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

18 crates, ~69,000 lines of Rust code, 824 tests.

---

## API Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/api/v1/query` | POST | Execute SQL queries (positional `$1` params) |
| `/api/v1/prepare` · `/prepared/execute` · `/prepared/:id` | POST/POST/DELETE | Prepared statements (prepare / execute / deallocate) |
| `/api/v1/tables` | GET | List all tables |
| `/api/v1/kv/keys` | GET/POST | List or set key-value pairs |
| `/api/v1/kv/keys/:key` | GET/DELETE | Get or delete a key |
| `/api/v1/kv/batch/{get,set,delete}` | POST | Batch KV operations |
| `/api/v1/documents/collections` | GET/POST | List or create collections |
| `/api/v1/documents/collections/:name/documents` | GET/POST/PUT/PATCH/DELETE | Document CRUD |
| `/api/v1/documents/collections/:name/query` | POST | Query (filter, sort, `cursor`/`next_cursor` pagination) |
| `/api/v1/documents/collections/:name/batch-{insert,delete}` | POST | Bulk document insert/delete |
| `/api/v1/timeseries/write` · `/query` | POST | Write / query time series |
| `/api/v1/graph/nodes` · `/nodes/:id` | POST/PUT/DELETE | Create / update / delete graph nodes |
| `/api/v1/graph/edges` · `/edges/:id` | POST/PUT/DELETE | Create / update / delete graph edges |
| `/api/v1/streaming/publish` | POST | Publish events |
| `/api/v1/streaming/channels/:channel/sse` | GET | Live SSE event stream |
| `/api/v1/vector/collections` · `/:name` | GET/POST · GET/DELETE | Vector collections (list/create/stats/drop) |
| `/api/v1/vector/collections/:name/upsert` · `/batch` | POST | Upsert one / many vectors |
| `/api/v1/vector/collections/:name/search` | POST | KNN search (HNSW) `{vector, k, filter}` |
| `/api/v1/fts/indexes` · `/:name` | GET/POST · GET/DELETE | Full-text indexes (list/create/stats/drop) |
| `/api/v1/fts/indexes/:name/documents` | POST | Index a document `{id, text, metadata}` |
| `/api/v1/fts/indexes/:name/search` | POST | BM25 search `{query, k, filter}` |
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

---

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=AutomataNexus/Aegis-DB&type=Date)](https://star-history.com/#AutomataNexus/Aegis-DB&Date)
