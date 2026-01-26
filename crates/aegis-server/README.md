<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/assets/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
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
- **TLS/HTTPS** - Native TLS support with certificate-based encryption
- **Authentication** - Username/password (Argon2id), MFA, LDAP, OAuth2/OIDC
- **Authorization** - Role-based access control (RBAC)
- **Rate Limiting** - Token bucket rate limiting for API protection
- **Secrets Management** - HashiCorp Vault integration
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
| `middleware` | Auth, logging, CORS, rate limiting middleware |
| `secrets` | HashiCorp Vault and environment secrets |
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
  -H, --host <HOST>        Host to bind to [default: 127.0.0.1]
  -p, --port <PORT>        Port to listen on [default: 9090]
  -d, --data-dir <DIR>     Data directory for persistence
      --node-id <ID>       Unique node ID
      --node-name <NAME>   Node display name
      --peers <PEERS>      Comma-separated peer addresses
      --cluster <NAME>     Cluster name [default: aegis-cluster]
      --tls                Enable TLS/HTTPS
      --tls-cert <PATH>    TLS certificate file (PEM)
      --tls-key <PATH>     TLS private key file (PEM)
  -h, --help               Print help
```

### TLS/HTTPS Mode

```bash
# With command-line arguments
cargo run -p aegis-server -- --tls --tls-cert /path/to/cert.pem --tls-key /path/to/key.pem

# With environment variables
export AEGIS_TLS_CERT=/path/to/cert.pem
export AEGIS_TLS_KEY=/path/to/key.pem
cargo run -p aegis-server -- --tls
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
enabled = true
cert_file = "/etc/aegis/tls.crt"
key_file = "/etc/aegis/tls.key"

[rate_limit]
requests_per_minute = 1000
login_requests_per_minute = 30
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `AEGIS_TLS_CERT` | Path to TLS certificate (PEM) |
| `AEGIS_TLS_KEY` | Path to TLS private key (PEM) |
| `VAULT_ADDR` | HashiCorp Vault server address |
| `VAULT_TOKEN` | Vault authentication token |
| `VAULT_ROLE_ID` | Vault AppRole role ID |
| `VAULT_SECRET_ID` | Vault AppRole secret ID |

### HashiCorp Vault Integration

For enterprise secrets management, the server integrates with HashiCorp Vault:

```bash
# Configure Vault
export VAULT_ADDR=https://vault.example.com:8200
export VAULT_TOKEN=hvs.your-token

# Secrets are read from: secret/data/aegis/*
# Falls back to environment variables if Vault unavailable
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

## Security Features

### Password Hashing
Passwords are hashed using Argon2id with:
- Memory cost: 19 MiB
- Time cost: 2 iterations
- Parallelism: 1
- Unique random salt per password

### Rate Limiting
Token bucket algorithm protects against brute force:
- Login endpoints: 30 requests/minute/IP
- General API: 1000 requests/minute/IP

### Session Tokens
Secure random tokens generated using `rand::thread_rng()` with cryptographically secure random number generation.

## Tests

```bash
cargo test -p aegis-server
```

**Test count:** 54 tests (unit + integration)

## License

Apache-2.0
