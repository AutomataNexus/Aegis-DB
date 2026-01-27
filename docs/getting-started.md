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

- **Rust** 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Trunk** (for dashboard): `cargo install trunk`

## Quick Installation

### From Binary Release

```bash
# Download latest release
curl -LO https://github.com/AutomataNexus/Aegis-DB/releases/latest/download/aegis-db-linux-x86_64.tar.gz

# Extract
tar -xzf aegis-db-linux-x86_64.tar.gz
cd aegis-db

# Run server
./aegis-server
```

### From Source

```bash
# Clone the repository
git clone https://github.com/AutomataNexus/Aegis-DB.git
cd Aegis-DB

# Build release
cargo build --release --workspace

# Run server
./target/release/aegis-server
```

### Build Dashboard (Optional)

```bash
cd crates/aegis-dashboard
trunk build --release
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|:---------|:------------|:--------|
| `AEGIS_DATA_DIR` | Data storage directory | `./data` |
| `AEGIS_PORT` | Server port | `9090` |
| `AEGIS_HOST` | Bind address | `0.0.0.0` |
| `AEGIS_LOG_LEVEL` | Logging level | `info` |
| `AEGIS_TLS_CERT` | TLS certificate path | (none) |
| `AEGIS_TLS_KEY` | TLS key path | (none) |

### Configuration File

Create `aegis.toml`:

```toml
[server]
host = "0.0.0.0"
port = 9090
data_dir = "/var/lib/aegis"

[auth]
enabled = true
jwt_secret = "your-secret-key-here"
session_timeout = 3600

[storage]
backend = "localfs"
compression = "lz4"

[replication]
enabled = false
mode = "single"
```

## Verify Installation

### Check Server Status

```bash
curl http://localhost:9090/health | jq
```

Expected response:
```json
{
  "status": "healthy",
  "version": "0.2.1",
  "uptime": 123
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
# Install CLI
cargo install --path crates/aegis-cli

# Connect to server
aegis-client connect localhost:9090

# Run queries
aegis-client query "CREATE TABLE users (id INT, name TEXT)"
aegis-client query "INSERT INTO users VALUES (1, 'Alice')"
aegis-client query "SELECT * FROM users"
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
