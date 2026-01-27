---
layout: default
title: aegis-server
parent: Crate Documentation
nav_order: 10
description: "REST API server"
---

# aegis-server

HTTP API server providing REST endpoints for database operations.

## Overview

The `aegis-server` crate implements the HTTP API layer for the Aegis database using the Axum web framework. It provides REST endpoints for query execution, table management, health checks, metrics, and comprehensive security features including TLS, rate limiting, and secrets management.

## Modules

### config.rs
Server configuration:

```rust
pub struct ServerConfig {
    pub host: String,           // Default: "127.0.0.1"
    pub port: u16,              // Default: 3000
    pub max_connections: usize, // Default: 10000
    pub request_timeout_secs: u64,
    pub body_limit_bytes: usize,
    pub enable_cors: bool,
    pub tls: Option<TlsConfig>,
}
```

### state.rs
Application state management:

**AppState:**
- Server configuration
- Query engine instance
- Metrics tracking

**QueryEngine:**
- SQL parser integration
- Query planner
- Execution context

**Metrics:**
- Total requests
- Failed requests
- Average duration
- Success rate

### handlers.rs
HTTP request handlers:

**Endpoints:**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| POST | `/api/v1/query` | Execute SQL query |
| GET | `/api/v1/tables` | List tables |
| GET | `/api/v1/tables/:name` | Get table details |
| GET | `/api/v1/metrics` | Server metrics |

**Query Request:**
```json
{
    "sql": "SELECT * FROM users WHERE age > 18",
    "params": []
}
```

**Query Response:**
```json
{
    "success": true,
    "data": {
        "columns": ["id", "name", "age"],
        "rows": [[1, "Alice", 30]],
        "rows_affected": 1
    },
    "execution_time_ms": 5
}
```

### router.rs
Axum router configuration:

- Route definitions
- Middleware stack
- CORS configuration
- Tracing layer

### middleware.rs
HTTP middleware:

**Request ID Middleware:**
- Generates unique request ID (UUID v4)
- Adds `x-request-id` header to request and response
- Enables request tracing

**Rate Limiting Middleware:**
- Token bucket algorithm
- Per-IP request tracking
- Configurable limits (default: 1000/min API, 30/min login)

**Authentication Middleware:**
- Bearer token validation
- Session management
- CORS configuration

### secrets.rs
Secrets management with HashiCorp Vault integration:

```rust
pub trait SecretsProvider: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn get_or(&self, key: &str, default: &str) -> String;
    fn exists(&self, key: &str) -> bool;
}

// Providers (checked in order):
// 1. HashiCorp Vault (if configured)
// 2. Environment variables
// 3. Default values
```

**Vault Authentication Methods:**
- Token-based (`VAULT_TOKEN`)
- AppRole (`VAULT_ROLE_ID` + `VAULT_SECRET_ID`)
- Kubernetes (`VAULT_KUBERNETES_ROLE`)

## Usage

```rust
use aegis_server::{ServerConfig, AppState, create_router};

#[tokio::main]
async fn main() {
    let config = ServerConfig::new("0.0.0.0", 3000);
    let state = AppState::new(config.clone());
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(config.socket_addr())
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
```

## API Examples

**Health Check:**
```bash
curl http://localhost:3000/health
```

**Execute Query:**
```bash
curl -X POST http://localhost:3000/api/v1/query \
  -H "Content-Type: application/json" \
  -d '{"sql": "SELECT 1 + 1 as result"}'
```

**Get Metrics:**
```bash
curl http://localhost:3000/api/v1/metrics
```

## TLS/HTTPS Configuration

```bash
# Enable TLS with command-line arguments
cargo run -p aegis-server -- \
  --tls \
  --tls-cert /path/to/server.crt \
  --tls-key /path/to/server.key

# Or use environment variables
export AEGIS_TLS_CERT=/path/to/server.crt
export AEGIS_TLS_KEY=/path/to/server.key
cargo run -p aegis-server -- --tls
```

## Security Features

| Feature | Implementation |
|---------|---------------|
| Password Hashing | Argon2id (19MB memory, 2 iterations) |
| Rate Limiting | Token bucket (1000/min API, 30/min login) |
| TLS | rustls with TLSv1.2/1.3 |
| Secrets | HashiCorp Vault + environment variables |
| Tokens | Cryptographically secure random generation |

## Tests

54 tests covering configuration, state management, HTTP endpoints, authentication, and rate limiting.
