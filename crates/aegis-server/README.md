<p align="center">
  <img src="../../AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-server

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.0-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

REST API server for the Aegis Database Platform.

## Overview

`aegis-server` provides the HTTP API layer built on Axum, featuring authentication, authorization, and endpoints for all database operations. It serves both the programmatic API and the web dashboard backend.

## Features

- **REST API** - Full CRUD operations for all data paradigms
- **Authentication** - Username/password, MFA, LDAP, OAuth2/OIDC
- **Authorization** - Role-based access control (RBAC)
- **Audit Logging** - Comprehensive activity tracking
- **Admin API** - Cluster management and monitoring

## Architecture

```
┌─────────────────────────────────────────────┐
│                HTTP Server                  │
│                  (Axum)                     │
├─────────────────────────────────────────────┤
│              Middleware Layer               │
│  ┌─────────┬──────────┬─────────────────┐  │
│  │  Auth   │  Logging │  Rate Limiting  │  │
│  └─────────┴──────────┴─────────────────┘  │
├─────────────────────────────────────────────┤
│               Route Handlers                │
│  ┌───────┬───────┬────────┬────────────┐   │
│  │ Query │  KV   │  Docs  │   Admin    │   │
│  └───────┴───────┴────────┴────────────┘   │
├─────────────────────────────────────────────┤
│            Application State                │
│         (Storage, Config, Auth)             │
└─────────────────────────────────────────────┘
```

## Modules

| Module | Description |
|--------|-------------|
| `router` | Route definitions and API structure |
| `handlers` | Request handlers for all endpoints |
| `auth` | Authentication providers (local, LDAP, OAuth2) |
| `middleware` | Auth, logging, CORS middleware |
| `admin` | Cluster administration endpoints |
| `activity` | Activity logging and audit trail |
| `state` | Shared application state |
| `config` | Server configuration |

## API Endpoints

### Core
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | Health check |
| POST | `/api/v1/query` | Execute SQL query |
| GET | `/api/v1/tables` | List tables |

### Authentication
| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/auth/login` | User login |
| POST | `/api/v1/auth/mfa/verify` | Verify MFA code |
| POST | `/api/v1/auth/logout` | User logout |
| GET | `/api/v1/auth/session` | Validate session |
| GET | `/api/v1/auth/me` | Current user info |

### Key-Value Store
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/kv/keys` | List keys |
| POST | `/api/v1/kv/keys` | Set key |
| DELETE | `/api/v1/kv/keys/:key` | Delete key |

### Documents
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/documents/collections` | List collections |
| GET | `/api/v1/documents/collections/:name` | Get documents |

### Admin
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/admin/cluster` | Cluster status |
| GET | `/api/v1/admin/nodes` | Node list |
| GET | `/api/v1/admin/stats` | Statistics |
| GET | `/api/v1/admin/alerts` | Active alerts |

## Usage

### Running the Server

```bash
# Using the CLI
aegis server start

# Or directly with cargo
cargo run -p aegis-server -- --port 9090

# With custom host
cargo run -p aegis-server -- --host 0.0.0.0 --port 9090
```

### Command-Line Options

```
aegis-server [OPTIONS]

Options:
  -H, --host <HOST>    Host to bind to [default: 127.0.0.1]
  -p, --port <PORT>    Port to listen on [default: 9090]
  -h, --help           Print help
```

### Configuration

```toml
[server]
host = "127.0.0.1"
port = 9090
max_connections = 1000
request_timeout = "30s"

[auth]
method = "local"           # local, ldap, oauth2
session_timeout = "30m"
mfa_required = false

[tls]
enabled = false
cert_file = "/etc/aegis/tls.crt"
key_file = "/etc/aegis/tls.key"
```

## Authentication

### Local Authentication
```bash
curl -X POST http://localhost:9090/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "demo", "password": "demo"}'
```

### Using the Token
```bash
curl http://localhost:9090/api/v1/admin/cluster \
  -H "Authorization: Bearer <token>"
```

## Tests

```bash
cargo test -p aegis-server
```

**Test count:** 38 tests + 23 E2E tests

## License

Apache-2.0
