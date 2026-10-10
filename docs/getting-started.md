---
layout: default
title: Getting Started
nav_order: 2
description: "Installation and setup guide for Aegis-DB"
---

# Getting Started
{: .no_toc }

## Table of Contents
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Prerequisites

Before installing Aegis-DB, ensure you have:

- **Rust** 1.85+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Trunk** (for dashboard): `cargo install trunk`

No OpenSSL or `libssl-dev` is needed: all TLS uses rustls.

## Quick Installation

### From Crates.io

```bash
# Install server
cargo install aegis-server

# Install CLI
cargo install aegisdb-cli
```

### From Binary Release

```bash
# Download the latest release binary and its checksums (v0.6.0)
curl -LO https://github.com/AutomataNexus/Aegis-DB/releases/download/v0.6.0/aegis-server-v0.6.0-linux-x86_64
curl -LO https://github.com/AutomataNexus/Aegis-DB/releases/download/v0.6.0/SHA256SUMS.txt
sha256sum --check --ignore-missing SHA256SUMS.txt

# Run server (default 127.0.0.1:9090)
chmod +x aegis-server-v0.6.0-linux-x86_64
./aegis-server-v0.6.0-linux-x86_64 --data-dir ./data
```

### From Source

```bash
# Clone the repository
git clone https://github.com/AutomataNexus/Aegis-DB.git
cd Aegis-DB

# Build release
cargo build --release --workspace

# Run server (default 127.0.0.1:9090)
./target/release/aegis-server --data-dir ./data
```

### Build Dashboard (Optional)

```bash
cd crates/aegis-dashboard
trunk build --release
```

## First Run: Provision an Admin

As of **v0.3.1** the server is **fail-closed**: with no users configured it
returns **503** on authenticated routes (only `/health` and
`/api/v1/auth/login` stay open) until you provision an admin. Set the admin
credentials before starting, and they are persisted on first boot:

```bash
export AEGIS_ADMIN_USERNAME=admin
export AEGIS_ADMIN_PASSWORD=your_secure_password
./aegis-server
```

If you need the legacy behavior where reads are served to anonymous callers
while no users exist (e.g. an internal proxy that talks to Aegis-DB
unauthenticated), opt in explicitly — every such request is logged as a
security warning:

```bash
export AEGIS_OPEN_BOOTSTRAP=true
```

## Configuration

The server is configured with command-line flags plus a few environment
variables. Run `aegis-server --help` for the full list.

### Command-Line Flags

| Flag | Description | Default |
|:-----|:------------|:--------|
| `-H, --host` | Bind address | `127.0.0.1` |
| `-p, --port` | Server port | `9090` |
| `-d, --data-dir` | Data directory; enables disk persistence | (in-memory) |
| `--node-id` / `--node-name` | Node identity in a cluster | auto-generated |
| `--peers` | Comma-separated peer addresses to join | (none) |
| `--cluster` | Cluster name | `aegis-cluster` |
| `--tls`, `--tls-cert`, `--tls-key` | Enable HTTPS with PEM cert and key | off |
| `--cluster-tls`, `--cluster-ca-cert` | TLS / mTLS for cluster traffic | off |

### Environment Variables

| Variable | Description | Default |
|:---------|:------------|:--------|
| `AEGIS_ADMIN_USERNAME` / `AEGIS_ADMIN_PASSWORD` | Admin provisioned on first boot | (none) |
| `AEGIS_TLS_CERT` / `AEGIS_TLS_KEY` | TLS cert / key paths when the flags are not given | (none) |
| `AEGIS_PRODUCTION` | Production mode (requires TLS unless `AEGIS_ALLOW_INSECURE`) | `false` |
| `AEGIS_CORS_ORIGINS` | Comma-separated allowed CORS origins | (none) |
| `AEGIS_TS_HOT_DAYS` | Time-series days kept in RAM | `7` |
| `AEGIS_TS_COLD_DAYS` | Time-series days kept in the on-disk cold tier (`0` disables) | `365` |
| `AEGIS_TS_MAX_QUERY_POINTS` | Most points one time-series query may return | `2000000` |
| `RUST_LOG` | Log filter (e.g. `info`, `aegis_server=debug`) | `info` |

See the [User Guide]({% link USER_GUIDE.md %}) and [Security]({% link SECURITY.md %})
for vault, encryption and LDAP/OAuth settings.

## Verify Installation

### Check Server Status

```bash
curl http://localhost:9090/health | jq
```

Expected response:
```json
{
  "status": "healthy",
  "version": "0.6.0"
}
```

### Test Query

```bash
curl -X POST http://localhost:9090/api/v1/query \
  -H "Content-Type: application/json" \
  -d '{"sql": "SELECT 1 + 1 AS result"}'
```

### Access Dashboard

Open [http://localhost:8000](http://localhost:8000) in your browser.

Default credentials: `demo` / `demo`

## Using the CLI

```bash
# Install CLI (from crates.io)
cargo install aegisdb-cli

# Or install from source
cargo install --path crates/aegis-cli

# Check server status (port 9090)
aegis-client -s http://localhost:9090 status

# Run queries (subcommand comes AFTER the database flag)
aegis-client -s http://localhost:9090 query "CREATE TABLE users (id INT, name TEXT)"
aegis-client -s http://localhost:9090 query "INSERT INTO users VALUES (1, 'Alice')"
aegis-client -s http://localhost:9090 query "SELECT * FROM users"

# Interactive shell
aegis-client -s http://localhost:9090 shell
```

## Using the Client SDK

### Rust

```rust
use aegis_client::AegisClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = AegisClient::connect("localhost:9090").await?;

    client.execute("CREATE TABLE users (id INT, name TEXT)").await?;
    client.execute("INSERT INTO users VALUES (1, 'Alice')").await?;

    let rows = client.query("SELECT * FROM users").await?;
    for row in rows {
        println!("{:?}", row);
    }

    Ok(())
}
```

### Python

```python
from aegis_client import AegisClient

client = AegisClient("localhost:9090")

client.execute("CREATE TABLE users (id INT, name TEXT)")
client.execute("INSERT INTO users VALUES (1, 'Alice')")

for row in client.query("SELECT * FROM users"):
    print(row)
```

### JavaScript

```javascript
import { AegisClient } from '@aegis-db/client';

const client = new AegisClient('localhost:9090');

await client.execute('CREATE TABLE users (id INT, name TEXT)');
await client.execute("INSERT INTO users VALUES (1, 'Alice')");

const rows = await client.query('SELECT * FROM users');
rows.forEach(row => console.log(row));
```

## PM2 Deployment (Production)

Create `ecosystem.config.js`:

```javascript
module.exports = {
  apps: [{
    name: 'aegis-server',
    script: './target/release/aegis-server',
    cwd: '/opt/Aegis-DB',
    env: {
      AEGIS_PORT: 9090,
      AEGIS_DATA_DIR: '/var/lib/aegis',
      RUST_LOG: 'info'
    }
  }]
};
```

Start with PM2:

```bash
pm2 start ecosystem.config.js
pm2 save
pm2 startup
```

## Next Steps

- [User Guide]({% link USER_GUIDE.md %}) - Full configuration reference
- [AegisQL Reference]({% link AegisQL.md %}) - Query language documentation
- [Security Guide]({% link SECURITY.md %}) - TLS, authentication, encryption
- [Crate Documentation]({% link crates/index.md %}) - Architecture deep dive
