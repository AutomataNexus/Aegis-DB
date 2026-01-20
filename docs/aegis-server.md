# aegis-server

HTTP API server providing REST endpoints for database operations.

## Overview

The `aegis-server` crate implements the HTTP API layer for the Aegis database using the Axum web framework. It provides REST endpoints for query execution, table management, health checks, and metrics.

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

## Tests

7 tests covering configuration, state management, and HTTP endpoints.
