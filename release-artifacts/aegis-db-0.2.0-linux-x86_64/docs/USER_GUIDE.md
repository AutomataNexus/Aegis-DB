<p align="center">
  <img src="../AegisDB-logo.png" alt="AegisDB Logo" width="400">
</p>

<h1 align="center">AegisDB End User Guide</h1>

<p align="center">
  <strong>Comprehensive guide for installing, configuring, and using AegisDB</strong>
</p>

---

## Table of Contents

1. [Introduction](#introduction)
2. [Installation](#installation)
   - [System Requirements](#system-requirements)
   - [Quick Install](#quick-install)
   - [Manual Installation](#manual-installation)
   - [Docker Installation](#docker-installation)
   - [Kubernetes Deployment](#kubernetes-deployment)
3. [Quick Start](#quick-start)
4. [Configuration](#configuration)
   - [Server Configuration](#server-configuration)
   - [Storage Configuration](#storage-configuration)
   - [Security Configuration](#security-configuration)
   - [Cluster Configuration](#cluster-configuration)
5. [Using the CLI](#using-the-cli)
   - [Service Management](#service-management)
   - [Interactive Shell](#interactive-shell)
   - [Query Execution](#query-execution)
   - [Data Import/Export](#data-importexport)
6. [Using the Web Dashboard](#using-the-web-dashboard)
   - [Login & Authentication](#login--authentication)
   - [Cluster Overview](#cluster-overview)
   - [Database Browser](#database-browser)
   - [Query Builder](#query-builder)
   - [Monitoring & Metrics](#monitoring--metrics)
   - [Administration](#administration)
7. [REST API Reference](#rest-api-reference)
   - [Authentication](#authentication)
   - [Query Endpoints](#query-endpoints)
   - [Key-Value Store](#key-value-store-api)
   - [Document Store](#document-store-api)
   - [Time Series](#time-series-api)
   - [Streaming](#streaming-api)
   - [Admin Endpoints](#admin-endpoints)
   - [Compliance APIs (GDPR, CCPA, HIPAA)](#compliance-apis-gdpr-ccpa-hipaa)
8. [Data Models](#data-models)
   - [SQL/Relational](#sqlrelational)
   - [Key-Value Store](#key-value-store)
   - [Document Store](#document-store)
   - [Time Series](#time-series)
   - [Streaming/Events](#streamingevents)
   - [Graph Data](#graph-data)
9. [AegisQL Query Language](#aegisql-query-language)
10. [Security & Authentication](#security--authentication)
    - [Local Authentication](#local-authentication)
    - [MFA/2FA](#mfa2fa)
    - [LDAP Integration](#ldap-integration)
    - [OAuth2/OIDC](#oauth2oidc)
    - [Role-Based Access Control](#role-based-access-control)
11. [Backup & Recovery](#backup--recovery)
12. [Monitoring & Alerting](#monitoring--alerting)
13. [Performance Tuning](#performance-tuning)
14. [Troubleshooting](#troubleshooting)
15. [FAQ](#faq)

---

## Introduction

AegisDB is a unified, multi-paradigm database platform built in Rust. It combines relational (SQL), time series, document, graph, and real-time streaming capabilities into a single high-performance system.

### Key Features

- **Multi-Paradigm Support**: SQL, Time Series, Document Store, Graph, and Streaming in one platform
- **Distributed Architecture**: Raft consensus, sharding, and multi-region replication
- **High Performance**: Vectorized execution, Gorilla compression, zero-copy serialization
- **Enterprise Security**: RBAC, audit logging, TLS, OAuth2/LDAP authentication
- **Modern Stack**: Rust backend, Leptos/WASM dashboard, comprehensive REST API

### Use Cases

| Use Case | Data Model | Example |
|----------|------------|---------|
| Traditional Applications | SQL/Relational | User management, orders, inventory |
| IoT & Monitoring | Time Series | Sensor data, metrics, logs |
| Content Management | Document Store | Articles, profiles, configurations |
| Real-time Analytics | Streaming | Event processing, CDC, notifications |
| Recommendations | Graph | Social networks, knowledge graphs |

---

## Installation

### System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 2 cores | 4+ cores |
| RAM | 2 GB | 8+ GB |
| Disk | 10 GB | 50+ GB SSD |
| OS | Linux, macOS, Windows | Linux (Ubuntu 22.04+) |

**Software Requirements:**
- Rust 1.75+ (for building from source)
- Trunk (for dashboard, `cargo install trunk`)

### Quick Install

```bash
# Clone the repository
git clone https://github.com/automatanexus/aegis-db.git
cd aegis-db

# Run the installer
./install.sh
```

The installer will:
1. Check for required tools (cargo, trunk)
2. Build the server and dashboard in release mode
3. Install the `aegis` command to `~/.local/bin`

After installation, add to your PATH if needed:
```bash
export PATH="$HOME/.local/bin:$PATH"
```

### Manual Installation

```bash
# Clone repository
git clone https://github.com/automatanexus/aegis-db.git
cd aegis-db

# Build all components
cargo build --release --workspace

# Build dashboard (requires trunk)
cd crates/aegis-dashboard
trunk build --release
cd ../..

# Copy binaries to desired location
cp target/release/aegis-server /usr/local/bin/
cp target/release/aegis /usr/local/bin/
```

### Docker Installation

```dockerfile
# Dockerfile
FROM rust:1.75-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p aegis-server

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/aegis-server /usr/local/bin/
EXPOSE 9090
CMD ["aegis-server", "--host", "0.0.0.0", "--port", "9090"]
```

```bash
# Build and run
docker build -t aegisdb:latest .
docker run -d -p 9090:9090 --name aegisdb aegisdb:latest
```

### Kubernetes Deployment

Helm charts are available in `deploy/helm/aegis-db/`:

```bash
# Add the Helm repository (if published)
helm repo add aegisdb https://charts.aegisdb.io

# Install with default values
helm install my-aegisdb aegisdb/aegis-db

# Or install from local chart
helm install my-aegisdb ./deploy/helm/aegis-db \
  --set replicaCount=3 \
  --set persistence.size=100Gi
```

---

## Quick Start

### 1. Start AegisDB

```bash
# Start server and dashboard
aegis start

# Check status
aegis status
```

### 2. Access the Dashboard

Open http://localhost:8000 in your browser.

**Credentials:**
Configure credentials via environment variables before starting:
```bash
export AEGIS_ADMIN_USERNAME=your_admin_username
export AEGIS_ADMIN_PASSWORD=your_secure_password
```

### 3. Run Your First Query

Using the CLI:
```bash
aegis-client -d dashboard query "SELECT 1 + 1 AS result"
```

Using curl:
```bash
curl -X POST http://localhost:9090/api/v1/query \
  -H "Content-Type: application/json" \
  -d '{"sql": "SELECT 1 + 1 AS result", "params": []}'
```

### 4. Store and Retrieve Data

**Key-Value:**
```bash
# Set a key
curl -X POST http://localhost:9090/api/v1/kv/keys \
  -H "Content-Type: application/json" \
  -d '{"key": "greeting", "value": "Hello, AegisDB!"}'

# Get the key
curl http://localhost:9090/api/v1/kv/keys/greeting
```

**Document:**
```bash
# Create a collection
curl -X POST http://localhost:9090/api/v1/documents/collections \
  -H "Content-Type: application/json" \
  -d '{"name": "users"}'

# Insert a document
curl -X POST http://localhost:9090/api/v1/documents/collections/users/documents \
  -H "Content-Type: application/json" \
  -d '{"document": {"name": "Alice", "email": "alice@example.com"}}'
```

---

## Configuration

AegisDB can be configured via configuration files, environment variables, or command-line arguments.

### Configuration File Location

```
~/.aegis/config.toml       # User configuration
/etc/aegis/aegis.toml      # System configuration
./aegis.toml               # Project configuration
```

### Server Configuration

```toml
[server]
host = "127.0.0.1"          # Bind address
port = 9090                  # HTTP port
max_connections = 1000       # Maximum concurrent connections
request_timeout = "30s"      # Request timeout
worker_threads = 0           # 0 = auto (number of CPUs)

[server.tls]
enabled = false
cert_file = "/etc/aegis/tls.crt"
key_file = "/etc/aegis/tls.key"
```

### Storage Configuration

```toml
[storage]
backend = "local"            # "memory" or "local"
data_directory = "/var/aegis/data"
compression = "lz4"          # "none", "lz4", "zstd"
buffer_pool_size = "1GB"
wal_enabled = true
sync_on_commit = true

[storage.retention]
default_duration = "30d"
enforce_interval = "1h"
```

### Security Configuration

```toml
[auth]
method = "local"             # "local", "ldap", "oauth2"
session_timeout = "30m"
mfa_required = false

[auth.ldap]
server_url = "ldap://ldap.example.com:389"
bind_dn = "cn=admin,dc=example,dc=com"
bind_password = "${LDAP_PASSWORD}"
base_dn = "dc=example,dc=com"
user_filter = "(uid={username})"
use_tls = true

[auth.oauth2]
provider_name = "okta"
client_id = "${OAUTH_CLIENT_ID}"
client_secret = "${OAUTH_CLIENT_SECRET}"
authorization_url = "https://example.okta.com/oauth2/v1/authorize"
token_url = "https://example.okta.com/oauth2/v1/token"
redirect_uri = "http://localhost:9090/callback"

[rbac]
enabled = true
default_role = "viewer"
```

### TLS/HTTPS Configuration

AegisDB supports native TLS for encrypted connections:

**Command Line:**
```bash
# Start with TLS enabled
cargo run -p aegis-server -- \
  --tls \
  --tls-cert /path/to/server.crt \
  --tls-key /path/to/server.key

# Or use environment variables
export AEGIS_TLS_CERT=/path/to/server.crt
export AEGIS_TLS_KEY=/path/to/server.key
cargo run -p aegis-server -- --tls
```

**Configuration File:**
```toml
[server.tls]
enabled = true
cert_file = "/etc/aegis/tls.crt"
key_file = "/etc/aegis/tls.key"
```

**Generating Self-Signed Certificates (Development):**
```bash
# Create certs directory
mkdir -p certs

# Generate self-signed certificate
openssl req -x509 -newkey rsa:4096 \
  -keyout certs/server.key \
  -out certs/server.crt \
  -days 365 -nodes \
  -subj "/CN=localhost/O=Aegis-DB/C=US"
```

### HashiCorp Vault Integration

For enterprise deployments, AegisDB integrates with HashiCorp Vault for secrets management:

**Environment Variables:**
```bash
# Vault server address
export VAULT_ADDR=https://vault.example.com:8200

# Token authentication (simplest)
export VAULT_TOKEN=hvs.your-vault-token

# AppRole authentication (recommended for production)
export VAULT_ROLE_ID=your-role-id
export VAULT_SECRET_ID=your-secret-id

# Kubernetes authentication (for K8s deployments)
export VAULT_KUBERNETES_ROLE=aegis-db
```

**Vault Secret Paths:**
| Secret | Vault Path | Environment Fallback |
|--------|------------|---------------------|
| TLS Certificate | `secret/data/aegis/tls_cert_path` | `AEGIS_TLS_CERT` |
| TLS Private Key | `secret/data/aegis/tls_key_path` | `AEGIS_TLS_KEY` |
| Database Password | `secret/data/aegis/db_password` | `AEGIS_DB_PASSWORD` |
| JWT Secret | `secret/data/aegis/jwt_secret` | `AEGIS_JWT_SECRET` |

**Vault Policy Example:**
```hcl
path "secret/data/aegis/*" {
  capabilities = ["read"]
}
```

### Rate Limiting

AegisDB includes built-in rate limiting to prevent brute force attacks:

| Endpoint | Default Limit | Window |
|----------|--------------|--------|
| `/api/v1/auth/login` | 30 requests | per minute per IP |
| All other endpoints | 1000 requests | per minute per IP |

Rate limit headers are included in responses:
- `X-RateLimit-Limit`: Maximum requests allowed
- `X-RateLimit-Remaining`: Requests remaining in window
- `X-RateLimit-Reset`: Unix timestamp when window resets

When rate limited, the API returns `429 Too Many Requests`.

### Cluster Configuration

```toml
[cluster]
node_id = 1
name = "aegis-cluster-1"

[cluster.peers]
nodes = [
  { id = 2, address = "node2.aegis.local:9091" },
  { id = 3, address = "node3.aegis.local:9091" },
]

[replication]
replication_factor = 3
consistency_level = "quorum"  # "one", "quorum", "all"

[sharding]
strategy = "hash"             # "hash" or "range"
shard_count = 16
auto_rebalance = true
```

### Environment Variables

All configuration options can be set via environment variables:

```bash
export AEGIS_SERVER_HOST=0.0.0.0
export AEGIS_SERVER_PORT=9090
export AEGIS_STORAGE_BACKEND=local
export AEGIS_AUTH_METHOD=ldap
export AEGIS_LDAP_PASSWORD=secret
```

---

## Using the CLI

The CLI tool is `aegis-client` (binary name: `aegis`). Build with `cargo build -p aegisdb-cli --release`.

### Connecting to a Database

```bash
# Using shorthand names
aegis-client -d nexusscribe query "SELECT 1"
aegis-client -d axonml query "SELECT * FROM users"
aegis-client -d dashboard status

# Using aegis:// URL
aegis-client -d aegis://localhost:9091/mydb query "SELECT 1"

# Using host:port
aegis-client -d localhost:7001 query "SELECT 1"

# Using server flag
aegis-client -s http://localhost:9091 query "SELECT 1"
```

**Shorthand Names:**
| Name | Port | Description |
|------|------|-------------|
| `dashboard` | 9090 | Main dashboard server |
| `local` | 9090 | Alias for dashboard |
| `axonml` | 7001 | AxonML node |
| `nexusscribe` | 9091 | NexusScribe node |

### Interactive Shell

```bash
# Start interactive SQL shell
aegis-client -d nexusscribe shell

# Connect to specific server
aegis-client -s http://db.example.com:9090 shell
```

**Shell Session:**

```
Aegis SQL Shell
Connected to: http://localhost:9091
Type 'exit' or 'quit' to exit, 'help' for commands.

aegis> SELECT * FROM users LIMIT 5;
+----+-------+-------------------+
| id | name  | email             |
+====+=======+===================+
| 1  | Alice | alice@example.com |
| 2  | Bob   | bob@example.com   |
+----+-------+-------------------+

2 row(s) returned in 0 ms

aegis> \d
(lists tables)

aegis> \h
Commands:
  \q, exit, quit  - Exit the shell
  \d              - List tables
  \h, help        - Show this help
  Any SQL         - Execute SQL query

aegis> \q
Bye!
```

### Query Execution

```bash
# Execute a single query
aegis-client -d nexusscribe query "SELECT * FROM users WHERE active = true"

# Output formats
aegis-client -d nexusscribe query "SELECT * FROM users" --format table
aegis-client -d nexusscribe query "SELECT * FROM users" --format json
aegis-client -d nexusscribe query "SELECT * FROM users" --format csv

# Save output to file
aegis-client -d nexusscribe query "SELECT * FROM users" -f json > users.json
```

### Key-Value Operations

```bash
# Set a key
aegis-client -d nexusscribe kv set mykey "myvalue"

# Get a key
aegis-client -d nexusscribe kv get mykey

# Delete a key
aegis-client -d nexusscribe kv delete mykey

# List all keys
aegis-client -d nexusscribe kv list

# List keys with prefix
aegis-client -d nexusscribe kv list --prefix "user:"
```

### Status and Metrics

```bash
# Server health check
aegis-client -d nexusscribe status

# Server metrics
aegis-client -d nexusscribe metrics

# Cluster information
aegis-client -d nexusscribe cluster

# List all tables
aegis-client -d nexusscribe tables
```

### Command Reference

```
aegis-client [OPTIONS] <COMMAND>

Commands:
  query    Execute a SQL query
  status   Check server health and status
  metrics  Get server metrics
  tables   List all tables
  kv       Key-value store operations
  cluster  Cluster information
  shell    Interactive SQL shell
  help     Print help

Options:
  -s, --server <SERVER>      Server URL [default: http://localhost:9090]
  -d, --database <DATABASE>  Database: shorthand (nexusscribe, axonml, dashboard),
                             URL (aegis://host:port/db), or host:port
  -h, --help                 Print help
  -V, --version              Print version
```

---

## Using the Web Dashboard

### Login & Authentication

1. Navigate to http://localhost:8000
2. Enter credentials configured via `AEGIS_ADMIN_USERNAME` and `AEGIS_ADMIN_PASSWORD` environment variables
3. If MFA is enabled, enter the 6-digit code from your authenticator app

**Note:** Configure credentials via environment variables before starting the server. MFA can be enabled for additional security.

### Cluster Overview

The Overview page displays:

- **Cluster Health**: Overall cluster status (Healthy/Degraded/Critical)
- **Node Status**: Visual indicators for each node
- **Key Metrics**:
  - Operations per second
  - Active connections
  - CPU/Memory/Disk usage
  - Replication lag
- **Recent Activity**: Real-time activity feed
- **Active Alerts**: Current warnings and errors

### Database Browser

**Key-Value Store:**
- View all keys with search and filtering
- Add new key-value pairs
- Edit existing values (JSON editor)
- Delete keys
- Bulk operations

**Document Collections:**
- Browse collections
- View documents with JSON syntax highlighting
- Create new collections
- Insert/update/delete documents
- Full-text search within collections

**Graph Visualization:**
- Interactive node-edge visualization
- Filter by node type
- Explore relationships
- Export graph data

### Query Builder

The Query Builder provides an interactive SQL interface:

1. **SQL Editor**: Syntax-highlighted editor with auto-complete
2. **Query History**: Access previously executed queries
3. **Results View**: Tabular display of query results
4. **Export**: Download results as CSV/JSON
5. **Explain Plan**: View query execution plan

**Example Queries:**
```sql
-- List all tables
SHOW TABLES;

-- Create a table
CREATE TABLE products (
    id INT PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    price DECIMAL(10, 2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Alter table (add, drop, rename columns)
ALTER TABLE products ADD COLUMN category VARCHAR(50);
ALTER TABLE products RENAME COLUMN name TO product_name;
ALTER TABLE products DROP COLUMN category;

-- Insert data
INSERT INTO products (id, name, price) VALUES (1, 'Widget', 19.99);

-- Query with joins
SELECT o.id, u.name, o.total
FROM orders o
JOIN users u ON o.user_id = u.id
WHERE o.created_at > '2024-01-01'
ORDER BY o.total DESC
LIMIT 10;
```

### Monitoring & Metrics

**Performance Tab:**
- Query throughput (ops/sec)
- Query latency (p50, p95, p99)
- Error rate
- Connection pool utilization

**Storage Tab:**
- Disk usage by database/collection
- WAL size and growth
- Compaction statistics
- Cache hit ratio

**Network Tab:**
- Request/response sizes
- Replication traffic
- Client connections by source

**Time Range Selection:**
- Last 1 hour
- Last 6 hours
- Last 24 hours
- Last 7 days
- Last 30 days
- Custom range

### Administration

**Cluster Management:**
- View/add/remove cluster nodes
- Initiate leader election
- Rebalance shards
- View Raft state

**User Management:**
- Create/edit/delete users
- Assign roles
- Enable/disable MFA
- View login history

**Security Settings:**
- Configure authentication method
- Manage RBAC roles and permissions
- View audit logs
- Export compliance reports

**Backup & Recovery:**
- Schedule automated backups
- Manual backup trigger
- Restore from backup
- Point-in-time recovery

---

## REST API Reference

Base URL: `http://localhost:9090`

### Authentication

All API endpoints (except `/health` and `/api/v1/auth/login`) require authentication.

**Login:**
```bash
# Use credentials from environment variables
curl -X POST http://localhost:9090/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d "{\"username\": \"$AEGIS_ADMIN_USERNAME\", \"password\": \"$AEGIS_ADMIN_PASSWORD\"}"
```

Response:
```json
{
  "token": "abc123...",
  "user": {
    "id": "user-001",
    "username": "your_username",
    "email": "user@example.com",
    "role": "admin",
    "mfa_enabled": false
  }
}
```

**Using the Token:**
```bash
curl http://localhost:9090/api/v1/admin/cluster \
  -H "Authorization: Bearer abc123..."
```

**MFA Verification (if required):**
```bash
curl -X POST http://localhost:9090/api/v1/auth/mfa/verify \
  -H "Content-Type: application/json" \
  -d '{"token": "temp-token", "code": "123456"}'
```

### Query Endpoints

**Execute SQL Query:**
```bash
POST /api/v1/query
Content-Type: application/json

{
  "sql": "SELECT * FROM users WHERE age > $1",
  "params": [21]
}
```

Response:
```json
{
  "success": true,
  "data": {
    "columns": ["id", "name", "age"],
    "rows": [
      [1, "Alice", 25],
      [2, "Bob", 30]
    ],
    "row_count": 2
  },
  "execution_time_ms": 12
}
```

**List Tables:**
```bash
GET /api/v1/tables
```

**Get Table Schema:**
```bash
GET /api/v1/tables/:name
```

### Key-Value Store API

**List Keys:**
```bash
GET /api/v1/kv/keys
GET /api/v1/kv/keys?prefix=user:
```

**Get Key:**
```bash
GET /api/v1/kv/keys/:key
```

**Set Key:**
```bash
POST /api/v1/kv/keys
Content-Type: application/json

{
  "key": "user:123",
  "value": {"name": "Alice", "email": "alice@example.com"}
}
```

**Delete Key:**
```bash
DELETE /api/v1/kv/keys/:key
```

### Document Store API

**List Collections:**
```bash
GET /api/v1/documents/collections
```

**Create Collection:**
```bash
POST /api/v1/documents/collections
Content-Type: application/json

{
  "name": "products"
}
```

**Get Documents:**
```bash
GET /api/v1/documents/collections/:name
```

Response:
```json
{
  "documents": [
    {
      "id": "doc-001",
      "collection": "products",
      "data": {"name": "Widget", "price": 19.99}
    }
  ],
  "total_scanned": 1,
  "execution_time_ms": 5
}
```

**Get Single Document:**
```bash
GET /api/v1/documents/collections/:name/documents/:id
```

**Insert Document:**
```bash
POST /api/v1/documents/collections/:name/documents
Content-Type: application/json

{
  "document": {
    "name": "Widget",
    "price": 19.99,
    "tags": ["electronics", "sale"]
  }
}
```

### Time Series API

**List Metrics:**
```bash
GET /api/v1/timeseries/metrics
```

**Register Metric:**
```bash
POST /api/v1/timeseries/metrics
Content-Type: application/json

{
  "name": "cpu_usage",
  "metric_type": "gauge",
  "description": "CPU usage percentage",
  "unit": "percent"
}
```

**Write Data Points:**
```bash
POST /api/v1/timeseries/write
Content-Type: application/json

{
  "metric": "cpu_usage",
  "points": [
    {"timestamp": "2024-01-15T10:00:00Z", "value": 45.5, "tags": {"host": "server-1"}},
    {"timestamp": "2024-01-15T10:01:00Z", "value": 48.2, "tags": {"host": "server-1"}}
  ]
}
```

**Query Time Series:**
```bash
POST /api/v1/timeseries/query
Content-Type: application/json

{
  "metric": "cpu_usage",
  "start_time": "2024-01-15T00:00:00Z",
  "end_time": "2024-01-15T23:59:59Z",
  "tags": {"host": "server-1"},
  "aggregation": {
    "function": "avg",
    "interval": "1h"
  }
}
```

### Streaming API

**List Channels:**
```bash
GET /api/v1/streaming/channels
```

**Create Channel:**
```bash
POST /api/v1/streaming/channels
Content-Type: application/json

{
  "name": "orders",
  "retention": "7d"
}
```

**Publish Event:**
```bash
POST /api/v1/streaming/publish
Content-Type: application/json

{
  "channel": "orders",
  "event_type": "order_created",
  "data": {
    "order_id": "ORD-123",
    "customer_id": "CUST-456",
    "total": 99.99
  }
}
```

**Get Channel History:**
```bash
GET /api/v1/streaming/channels/:channel/history?limit=100
```

### Admin Endpoints

**Cluster Info:**
```bash
GET /api/v1/admin/cluster
```

**Dashboard Summary:**
```bash
GET /api/v1/admin/dashboard
```

**List Nodes:**
```bash
GET /api/v1/admin/nodes
```

**Node Operations:**
```bash
POST /api/v1/admin/nodes/:node_id/restart
POST /api/v1/admin/nodes/:node_id/drain
DELETE /api/v1/admin/nodes/:node_id
GET /api/v1/admin/nodes/:node_id/logs
```

**Storage Info:**
```bash
GET /api/v1/admin/storage
```

**Query Statistics:**
```bash
GET /api/v1/admin/stats
```

**Alerts:**
```bash
GET /api/v1/admin/alerts
```

**Activity Log:**
```bash
GET /api/v1/admin/activities?limit=50&type=query
```

### Compliance APIs (GDPR, CCPA, HIPAA)

AegisDB provides built-in compliance APIs to help organizations meet regulatory requirements for data protection and privacy. These endpoints support GDPR (EU), CCPA (California), and HIPAA (US Healthcare) compliance workflows.

#### Data Subject Rights (GDPR Article 17 & 20)

**Right to Erasure (Delete Personal Data):**

Permanently delete all data associated with a data subject. Returns a cryptographic deletion certificate for audit purposes.

```bash
DELETE /api/v1/compliance/data-subject/{identifier}
```

Example:
```bash
curl -X DELETE http://localhost:9090/api/v1/compliance/data-subject/user@example.com \
  -H "Authorization: Bearer $TOKEN"
```

Response:
```json
{
  "success": true,
  "certificate_id": "cert-a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "subject_identifier": "user@example.com",
  "deleted_at": "2024-01-15T10:30:00Z",
  "records_deleted": 47,
  "tables_affected": ["users", "orders", "activity_logs"]
}
```

**Data Portability (Export Personal Data):**

Export all data associated with a data subject in a machine-readable format.

```bash
POST /api/v1/compliance/export
Content-Type: application/json

{
  "subject_identifier": "user@example.com",
  "format": "json",
  "include_metadata": true
}
```

Example:
```bash
curl -X POST http://localhost:9090/api/v1/compliance/export \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "subject_identifier": "user@example.com",
    "format": "json",
    "include_metadata": true
  }'
```

Response:
```json
{
  "success": true,
  "export_id": "export-12345",
  "subject_identifier": "user@example.com",
  "format": "json",
  "download_url": "/api/v1/compliance/export/export-12345/download",
  "expires_at": "2024-01-16T10:30:00Z",
  "size_bytes": 245678,
  "record_count": 156
}
```

**List Deletion Certificates:**

Retrieve all deletion certificates for audit and compliance reporting.

```bash
GET /api/v1/compliance/certificates
GET /api/v1/compliance/certificates?from=2024-01-01&to=2024-01-31
```

Example:
```bash
curl http://localhost:9090/api/v1/compliance/certificates \
  -H "Authorization: Bearer $TOKEN"
```

Response:
```json
{
  "certificates": [
    {
      "id": "cert-a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      "subject_identifier": "user@example.com",
      "deleted_at": "2024-01-15T10:30:00Z",
      "deleted_by": "admin",
      "records_deleted": 47,
      "verification_hash": "sha256:abc123..."
    }
  ],
  "total_count": 1
}
```

**Verify Deletion Certificate:**

Cryptographically verify the authenticity of a deletion certificate.

```bash
GET /api/v1/compliance/certificates/{id}/verify
```

Example:
```bash
curl http://localhost:9090/api/v1/compliance/certificates/cert-a1b2c3d4-e5f6-7890-abcd-ef1234567890/verify \
  -H "Authorization: Bearer $TOKEN"
```

Response:
```json
{
  "valid": true,
  "certificate_id": "cert-a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "verified_at": "2024-01-20T14:00:00Z",
  "signature_algorithm": "Ed25519",
  "issuer": "aegisdb-compliance-service"
}
```

#### Consent Management

**Record Consent:**

Record a data subject's consent for a specific processing purpose.

```bash
POST /api/v1/compliance/consent
Content-Type: application/json

{
  "subject_id": "user@example.com",
  "purpose": "marketing_emails",
  "granted": true,
  "lawful_basis": "consent",
  "expiry": "2025-01-15T00:00:00Z",
  "metadata": {
    "ip_address": "192.168.1.1",
    "user_agent": "Mozilla/5.0...",
    "consent_form_version": "2.1"
  }
}
```

Example:
```bash
curl -X POST http://localhost:9090/api/v1/compliance/consent \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "subject_id": "user@example.com",
    "purpose": "marketing_emails",
    "granted": true,
    "lawful_basis": "consent",
    "expiry": "2025-01-15T00:00:00Z"
  }'
```

Response:
```json
{
  "success": true,
  "consent_id": "consent-789xyz",
  "subject_id": "user@example.com",
  "purpose": "marketing_emails",
  "granted": true,
  "recorded_at": "2024-01-15T10:30:00Z"
}
```

**Get Consent Status:**

Retrieve all consent records for a data subject.

```bash
GET /api/v1/compliance/consent/{subject_id}
```

Example:
```bash
curl http://localhost:9090/api/v1/compliance/consent/user@example.com \
  -H "Authorization: Bearer $TOKEN"
```

Response:
```json
{
  "subject_id": "user@example.com",
  "consents": [
    {
      "consent_id": "consent-789xyz",
      "purpose": "marketing_emails",
      "granted": true,
      "lawful_basis": "consent",
      "granted_at": "2024-01-15T10:30:00Z",
      "expiry": "2025-01-15T00:00:00Z",
      "active": true
    },
    {
      "consent_id": "consent-456abc",
      "purpose": "analytics",
      "granted": true,
      "lawful_basis": "legitimate_interest",
      "granted_at": "2024-01-10T08:00:00Z",
      "expiry": null,
      "active": true
    }
  ]
}
```

**Withdraw Consent:**

Withdraw a data subject's consent for a specific purpose.

```bash
DELETE /api/v1/compliance/consent/{subject_id}/{purpose}
```

Example:
```bash
curl -X DELETE http://localhost:9090/api/v1/compliance/consent/user@example.com/marketing_emails \
  -H "Authorization: Bearer $TOKEN"
```

Response:
```json
{
  "success": true,
  "subject_id": "user@example.com",
  "purpose": "marketing_emails",
  "withdrawn_at": "2024-01-20T14:00:00Z",
  "previous_consent_id": "consent-789xyz"
}
```

**CCPA Do Not Sell List:**

Retrieve the list of data subjects who have opted out of data sales (CCPA requirement).

```bash
GET /api/v1/compliance/do-not-sell
GET /api/v1/compliance/do-not-sell?page=1&limit=100
```

Example:
```bash
curl http://localhost:9090/api/v1/compliance/do-not-sell \
  -H "Authorization: Bearer $TOKEN"
```

Response:
```json
{
  "do_not_sell": [
    {
      "subject_id": "user1@example.com",
      "opted_out_at": "2024-01-10T12:00:00Z",
      "verification_method": "authenticated_request"
    },
    {
      "subject_id": "user2@example.com",
      "opted_out_at": "2024-01-12T15:30:00Z",
      "verification_method": "email_confirmation"
    }
  ],
  "total_count": 2,
  "page": 1,
  "limit": 100
}
```

#### Breach Detection

**List Security Incidents:**

Retrieve detected security incidents and potential data breaches.

```bash
GET /api/v1/compliance/breaches
GET /api/v1/compliance/breaches?status=open&severity=high
```

Example:
```bash
curl http://localhost:9090/api/v1/compliance/breaches \
  -H "Authorization: Bearer $TOKEN"
```

Response:
```json
{
  "incidents": [
    {
      "id": "breach-001",
      "detected_at": "2024-01-15T03:45:00Z",
      "severity": "high",
      "status": "open",
      "type": "unauthorized_access",
      "description": "Multiple failed login attempts followed by successful access from unusual IP",
      "affected_records_estimate": 150,
      "affected_subjects_estimate": 50,
      "source_ip": "203.0.113.42",
      "acknowledged": false
    }
  ],
  "total_count": 1,
  "open_count": 1,
  "acknowledged_count": 0
}
```

**Acknowledge Incident:**

Acknowledge a security incident to indicate it has been reviewed.

```bash
POST /api/v1/compliance/breaches/{id}/acknowledge
Content-Type: application/json

{
  "acknowledged_by": "security_admin",
  "notes": "Investigated and confirmed as false positive - authorized penetration test",
  "classification": "false_positive"
}
```

Example:
```bash
curl -X POST http://localhost:9090/api/v1/compliance/breaches/breach-001/acknowledge \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "acknowledged_by": "security_admin",
    "notes": "Investigated and confirmed as false positive - authorized penetration test",
    "classification": "false_positive"
  }'
```

Response:
```json
{
  "success": true,
  "incident_id": "breach-001",
  "acknowledged_at": "2024-01-15T10:00:00Z",
  "acknowledged_by": "security_admin",
  "classification": "false_positive",
  "status": "acknowledged"
}
```

**Generate Compliance Report:**

Generate a detailed compliance report for a specific incident, suitable for regulatory notification.

```bash
GET /api/v1/compliance/breaches/{id}/report
GET /api/v1/compliance/breaches/{id}/report?format=pdf
```

Example:
```bash
curl http://localhost:9090/api/v1/compliance/breaches/breach-001/report \
  -H "Authorization: Bearer $TOKEN"
```

Response:
```json
{
  "report_id": "report-breach-001-20240115",
  "incident_id": "breach-001",
  "generated_at": "2024-01-15T10:30:00Z",
  "report_type": "breach_notification",
  "incident_summary": {
    "detected_at": "2024-01-15T03:45:00Z",
    "severity": "high",
    "type": "unauthorized_access",
    "description": "Multiple failed login attempts followed by successful access from unusual IP"
  },
  "impact_assessment": {
    "affected_records": 150,
    "affected_subjects": 50,
    "data_categories": ["email", "name", "purchase_history"],
    "sensitive_data_involved": false
  },
  "timeline": [
    {"timestamp": "2024-01-15T03:30:00Z", "event": "First failed login attempt detected"},
    {"timestamp": "2024-01-15T03:45:00Z", "event": "Successful login from same IP after 15 attempts"},
    {"timestamp": "2024-01-15T03:45:30Z", "event": "Anomaly detection triggered alert"}
  ],
  "remediation_actions": [
    "Account temporarily locked",
    "IP address blocked",
    "Password reset required"
  ],
  "regulatory_requirements": {
    "gdpr_notification_required": true,
    "gdpr_notification_deadline": "2024-01-18T03:45:00Z",
    "ccpa_notification_required": true,
    "hipaa_notification_required": false
  },
  "download_url": "/api/v1/compliance/breaches/breach-001/report/download"
}
```

---

## Data Models

### SQL/Relational

AegisDB supports standard SQL with ACID transactions.

**Supported Data Types:**
| Type | Description |
|------|-------------|
| `INT` / `INTEGER` | 32-bit integer |
| `BIGINT` | 64-bit integer |
| `FLOAT` / `REAL` | 32-bit floating point |
| `DOUBLE` | 64-bit floating point |
| `DECIMAL(p,s)` | Exact decimal |
| `VARCHAR(n)` | Variable-length string |
| `TEXT` | Unlimited text |
| `BOOLEAN` | True/false |
| `DATE` | Date (YYYY-MM-DD) |
| `TIME` | Time (HH:MM:SS) |
| `TIMESTAMP` | Date and time |
| `JSON` | JSON document |
| `BLOB` | Binary data |

**Example:**
```sql
CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    customer_id INT NOT NULL REFERENCES customers(id),
    status VARCHAR(20) DEFAULT 'pending',
    total DECIMAL(10, 2),
    items JSON,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO orders (customer_id, total, items)
VALUES (1, 99.99, '[{"sku": "ABC", "qty": 2}]');

SELECT o.*, c.name as customer_name
FROM orders o
JOIN customers c ON o.customer_id = c.id
WHERE o.status = 'pending'
ORDER BY o.created_at DESC;
```

### Key-Value Store

Simple key-value storage for fast lookups and caching.

**Characteristics:**
- Keys: String (max 1KB)
- Values: Any JSON-serializable data (max 1MB default)
- TTL support for automatic expiration
- Prefix-based listing and deletion

**Use Cases:**
- Session storage
- Configuration caching
- Feature flags
- Rate limiting counters

### Document Store

Schema-optional JSON document storage with powerful querying.

**Features:**
- Collections for logical grouping
- Secondary indexes on any field
- Full-text search
- JSONPath queries
- Aggregation pipeline

**Query Operators:**
| Operator | Description | Example |
|----------|-------------|---------|
| `$eq` | Equals | `{"status": {"$eq": "active"}}` |
| `$ne` | Not equals | `{"status": {"$ne": "deleted"}}` |
| `$gt`, `$gte` | Greater than | `{"age": {"$gt": 21}}` |
| `$lt`, `$lte` | Less than | `{"price": {"$lt": 100}}` |
| `$in` | In array | `{"status": {"$in": ["active", "pending"]}}` |
| `$regex` | Pattern match | `{"email": {"$regex": "@company\\.com$"}}` |
| `$and`, `$or` | Logical | `{"$or": [{"a": 1}, {"b": 2}]}` |

### Time Series

Optimized storage for time-stamped data with compression.

**Data Model:**
```
Metric: cpu_usage
  └── Tags: {host: "server-1", dc: "us-east"}
      └── Timestamp: 2024-01-15T10:00:00Z
          └── Value: 45.5
```

**Aggregation Functions:**
- `sum`, `avg`, `min`, `max`, `count`
- `first`, `last`
- `stddev`, `variance`
- `percentile(n)`

**Retention Policies:**
- Automatic data expiration
- Downsampling (e.g., 1-minute → 1-hour after 7 days)
- Tiered storage

### Streaming/Events

Real-time event streaming with persistent storage.

**Concepts:**
- **Channels**: Named event streams
- **Events**: Timestamped messages with type and payload
- **Subscriptions**: Durable or ephemeral consumers
- **Consumer Groups**: Load-balanced consumption

**Delivery Guarantees:**
- At-most-once (fire and forget)
- At-least-once (acknowledged delivery)
- Exactly-once (with transactions)

### Graph Data

Node and edge relationships for connected data.

**Model:**
```
Node: {id: "user:1", label: "User", properties: {name: "Alice"}}
Edge: {from: "user:1", to: "user:2", label: "FOLLOWS", properties: {since: "2024-01-01"}}
```

**Query Example:**
```sql
-- Find friends of friends
MATCH (u:User {name: 'Alice'})-[:FOLLOWS]->(friend)-[:FOLLOWS]->(fof)
WHERE fof <> u
RETURN DISTINCT fof.name
```

---

## AegisQL Query Language

AegisDB supports standard SQL plus extensions for multi-paradigm queries. See [AegisQL Reference](AegisQL.md) for complete documentation.

**SQL Standard Features:**
- SELECT, INSERT, UPDATE, DELETE
- JOINs (INNER, LEFT, RIGHT, FULL)
- Subqueries and CTEs
- Window functions
- Aggregations with GROUP BY / HAVING

**Time Series Extensions:**
```sql
-- Query time series with downsampling
SELECT
    time_bucket('1 hour', timestamp) as hour,
    avg(value) as avg_cpu
FROM metrics
WHERE metric_name = 'cpu_usage'
  AND timestamp > NOW() - INTERVAL '24 hours'
GROUP BY hour
ORDER BY hour;
```

**Document Extensions:**
```sql
-- Query JSON fields
SELECT
    data->>'name' as name,
    data->'address'->>'city' as city
FROM documents
WHERE collection = 'users'
  AND data @> '{"active": true}';
```

---

## Security & Authentication

### Local Authentication

Default authentication using username/password stored in AegisDB.

**Configuring Users:**
Set initial admin credentials via environment variables:
```bash
export AEGIS_ADMIN_USERNAME=admin
export AEGIS_ADMIN_PASSWORD=your_secure_password
```

**Available Roles:**
| Role | Description |
|------|-------------|
| Admin | Full access, can manage users and configure system |
| Operator | Can execute queries and manage databases |
| Viewer | Read-only access |

**Creating Users:**
```bash
# Via CLI (future)
aegis user create --username alice --email alice@example.com --role operator

# Via API
curl -X POST http://localhost:9090/api/v1/admin/users \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "email": "alice@example.com", "password": "secret", "role": "operator"}'
```

### MFA/2FA

Multi-factor authentication using TOTP (Time-based One-Time Password).

**Setup:**
1. Login to dashboard as the user
2. Go to Settings → Security
3. Click "Enable 2FA"
4. Scan QR code with authenticator app (Google Authenticator, Authy, etc.)
5. Enter verification code to confirm

**Testing:**
For development, use code `123456` which is always accepted.

### LDAP Integration

Connect to enterprise directory services.

```toml
[auth]
method = "ldap"

[auth.ldap]
server_url = "ldaps://ldap.company.com:636"
bind_dn = "cn=aegis-service,ou=services,dc=company,dc=com"
bind_password = "${LDAP_BIND_PASSWORD}"
base_dn = "ou=users,dc=company,dc=com"
user_filter = "(&(objectClass=person)(uid={username}))"
group_filter = "(member={dn})"
use_tls = true
admin_groups = ["cn=db-admins,ou=groups,dc=company,dc=com"]
operator_groups = ["cn=db-operators,ou=groups,dc=company,dc=com"]
```

### OAuth2/OIDC

Integrate with identity providers like Okta, Auth0, Azure AD.

```toml
[auth]
method = "oauth2"

[auth.oauth2]
provider_name = "okta"
client_id = "${OAUTH_CLIENT_ID}"
client_secret = "${OAUTH_CLIENT_SECRET}"
authorization_url = "https://company.okta.com/oauth2/v1/authorize"
token_url = "https://company.okta.com/oauth2/v1/token"
userinfo_url = "https://company.okta.com/oauth2/v1/userinfo"
redirect_uri = "https://aegis.company.com/callback"
scopes = ["openid", "profile", "email", "groups"]
role_claim = "groups"
admin_roles = ["AegisDB-Admins"]
operator_roles = ["AegisDB-Operators"]
```

### Role-Based Access Control

**Built-in Roles:**

| Role | Description | Key Permissions |
|------|-------------|-----------------|
| Admin | Full access | All operations including user management |
| Operator | Database operations | CRUD, backups, view metrics |
| Analyst | Read + analytics | SELECT, view metrics and logs |
| Viewer | Read-only | SELECT only |

**Permissions:**
- Database: `CREATE`, `DROP`, `LIST`
- Table: `CREATE`, `DROP`, `ALTER`, `LIST`
- Data: `SELECT`, `INSERT`, `UPDATE`, `DELETE`
- Admin: `USER_CREATE`, `USER_DELETE`, `ROLE_ASSIGN`
- System: `CONFIG_VIEW`, `CONFIG_MODIFY`, `METRICS_VIEW`, `LOGS_VIEW`
- Cluster: `NODE_ADD`, `NODE_REMOVE`, `BACKUP_CREATE`, `BACKUP_RESTORE`

**Row-Level Security:**
```sql
-- Only allow users to see their own data
CREATE POLICY user_isolation ON orders
  FOR SELECT
  USING (user_id = current_user_id());
```

---

## Backup & Recovery

### Manual Backup

```bash
# Full backup
aegis backup create --output /backups/aegis-$(date +%Y%m%d).tar.gz

# Incremental backup
aegis backup create --incremental --output /backups/aegis-incr-$(date +%Y%m%d%H%M).tar.gz
```

### Scheduled Backups

Via configuration:
```toml
[backup]
enabled = true
schedule = "0 2 * * *"  # Daily at 2 AM
retention_days = 30
destination = "/backups"
compression = "gzip"
```

### Restore

```bash
# List available backups
aegis backup list

# Restore from backup
aegis backup restore --from /backups/aegis-20240115.tar.gz

# Point-in-time recovery
aegis backup restore --from /backups/aegis-20240115.tar.gz --to-time "2024-01-15T10:30:00Z"
```

---

## Monitoring & Alerting

### Prometheus Integration

Metrics are exposed at `/metrics` in Prometheus format:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'aegisdb'
    static_configs:
      - targets: ['aegis-server:9090']
    metrics_path: /metrics
```

### Key Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `aegis_requests_total` | Counter | Total HTTP requests |
| `aegis_request_duration_seconds` | Histogram | Request latency |
| `aegis_queries_total` | Counter | Total queries executed |
| `aegis_query_duration_seconds` | Histogram | Query latency |
| `aegis_connections_active` | Gauge | Active connections |
| `aegis_storage_bytes` | Gauge | Storage used |
| `aegis_cache_hits_total` | Counter | Cache hits |
| `aegis_replication_lag_seconds` | Gauge | Replication lag |

### Alerting Rules

Example Prometheus alerting rules:

```yaml
groups:
  - name: aegisdb
    rules:
      - alert: HighQueryLatency
        expr: histogram_quantile(0.99, rate(aegis_query_duration_seconds_bucket[5m])) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High query latency detected"

      - alert: HighErrorRate
        expr: rate(aegis_requests_total{status="error"}[5m]) / rate(aegis_requests_total[5m]) > 0.05
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Error rate above 5%"
```

### Grafana Dashboards

Pre-built dashboards are available in `integrations/grafana-datasource/`:

1. **AegisDB Overview**: Cluster health, throughput, latency
2. **Query Performance**: Query metrics, slow query analysis
3. **Storage Metrics**: Disk usage, compaction, WAL
4. **Replication Status**: Raft state, lag, throughput

---

## Performance Tuning

### Query Optimization

**Use EXPLAIN to analyze queries:**
```sql
EXPLAIN SELECT * FROM orders WHERE customer_id = 123;
EXPLAIN ANALYZE SELECT * FROM orders WHERE customer_id = 123;
```

**Create indexes for frequent queries:**
```sql
CREATE INDEX idx_orders_customer ON orders(customer_id);
CREATE INDEX idx_orders_status_date ON orders(status, created_at);
```

### Connection Pooling

Configure appropriate pool sizes:
```toml
[server]
max_connections = 1000

[connection_pool]
min_connections = 10
max_connections = 100
idle_timeout = "300s"
```

### Memory Configuration

```toml
[storage]
buffer_pool_size = "4GB"      # Adjust based on available RAM
write_buffer_size = "64MB"

[query]
max_memory_per_query = "256MB"
sort_buffer_size = "64MB"
```

### Compression Settings

```toml
[storage]
compression = "lz4"           # Fast compression
# compression = "zstd"        # Better ratio, slower

[timeseries]
compression = "gorilla"       # Best for time series
```

---

## Troubleshooting

### Common Issues

**Cannot connect to server:**
```bash
# Check if server is running
aegis status

# Check port binding
netstat -tlnp | grep 9090

# Check logs
aegis logs server
```

**Authentication failures:**
```bash
# Verify credentials (use your configured credentials)
curl -X POST http://localhost:9090/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d "{\"username\": \"$AEGIS_ADMIN_USERNAME\", \"password\": \"$AEGIS_ADMIN_PASSWORD\"}"

# Check auth configuration
cat ~/.aegis/config.toml | grep -A10 "\[auth\]"
```

**Slow queries:**
```sql
-- Enable query logging
SET log_queries = true;
SET log_slow_queries = true;
SET slow_query_threshold = '100ms';

-- Analyze query
EXPLAIN ANALYZE SELECT ...;
```

**Out of memory:**
```toml
# Reduce memory usage
[storage]
buffer_pool_size = "1GB"

[query]
max_memory_per_query = "128MB"
```

### Log Locations

| Log | Location |
|-----|----------|
| Server | `~/.aegis/logs/server.log` |
| Dashboard | `~/.aegis/logs/dashboard.log` |
| Queries | `~/.aegis/logs/queries.log` |
| Audit | `~/.aegis/logs/audit.log` |

### Debug Mode

```bash
# Start server in debug mode
RUST_LOG=debug aegis server start

# Verbose CLI output
aegis --verbose query "SELECT 1"
```

### Health Check

```bash
# Basic health
curl http://localhost:9090/health

# Detailed health
curl http://localhost:9090/health/detailed
```

---

## FAQ

### General

**Q: What databases can AegisDB replace?**
A: AegisDB can serve as a replacement or complement for:
- PostgreSQL/MySQL (relational)
- InfluxDB/TimescaleDB (time series)
- MongoDB/CouchDB (documents)
- Redis (key-value, caching)
- Kafka (streaming)

**Q: Is AegisDB production-ready?**
A: Yes, AegisDB v0.1.8+ includes production security features: TLS/HTTPS support, Argon2id password hashing, rate limiting, HashiCorp Vault integration, and secure token generation. It's suitable for production deployments with proper configuration.

**Q: What's the license?**
A: Apache 2.0 for the core platform. Enterprise features may require a commercial license.

### Performance

**Q: How many operations per second can AegisDB handle?**
A: Benchmarks show:
- Key-Value: ~500K ops/sec (memory backend)
- SQL queries: ~10K simple queries/sec
- Time series writes: ~100K points/sec

**Q: How does compression affect performance?**
A: LZ4 adds ~5% CPU overhead with 2-4x compression. Zstd adds ~15% overhead with 4-10x compression. Gorilla compression for time series achieves ~12x with minimal overhead.

### Clustering

**Q: How many nodes are recommended?**
A: Minimum 3 nodes for fault tolerance. We recommend 5 nodes for production to handle 2 simultaneous failures.

**Q: What happens during a network partition?**
A: AegisDB uses Raft consensus. The partition with a majority of nodes continues operating. The minority partition becomes read-only until connectivity is restored.

### Security

**Q: Is data encrypted at rest?**
A: Encryption at rest is planned for v0.2. Currently, rely on filesystem-level encryption (LUKS, BitLocker).

**Q: How are passwords stored?**
A: Passwords are hashed using Argon2id, the winner of the Password Hashing Competition. Each password has a unique random salt, and the hash parameters are tuned for security (memory cost: 19MB, time cost: 2 iterations, parallelism: 1).

---

## Getting Help

- **Documentation**: https://docs.aegisdb.io
- **GitHub Issues**: https://github.com/automatanexus/aegis-db/issues
- **Discord**: Join our community chat
- **Stack Overflow**: Tag questions with `aegis-db`
- **Email**: Devops@automatanexus.com

---

<p align="center">
  <strong>AegisDB - The Multi-Paradigm Database Platform</strong><br>
  Copyright 2024-2026 Andrew Jewell Sr / AutomataNexus LLC
</p>
