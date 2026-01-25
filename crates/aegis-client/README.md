<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/Aegis-DB/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-client

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.7-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Client SDK for the Aegis Database Platform.

## Overview

`aegis-client` provides a Rust client library for connecting to Aegis servers. It features connection pooling, automatic reconnection, query building, and transaction support.

## Features

- **Connection Pooling** - Efficient connection management
- **Async/Await** - Full async support with Tokio
- **Query Builder** - Type-safe query construction
- **Transactions** - ACID transaction support
- **Automatic Retry** - Configurable retry policies

## Modules

| Module | Description |
|--------|-------------|
| `connection` | Connection management |
| `pool` | Connection pooling |
| `query` | Query builder |
| `transaction` | Transaction handling |
| `result` | Query result types |
| `error` | Error types |
| `config` | Client configuration |

## Usage

```toml
[dependencies]
aegis-client = { path = "../aegis-client" }
```

### Basic Example

```rust
use aegis_client::{AegisClient, ClientConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let config = ClientConfig::new("localhost:9090")
        .with_auth("demo", "demo");

    let client = AegisClient::connect(config).await?;

    // Execute a query
    let result = client.query("SELECT * FROM users").await?;

    for row in result.rows() {
        println!("User: {:?}", row);
    }

    Ok(())
}
```

### Connection Pooling

```rust
use aegis_client::{AegisClient, PoolConfig};

let pool_config = PoolConfig::default()
    .min_connections(5)
    .max_connections(20)
    .idle_timeout(Duration::from_secs(300));

let client = AegisClient::with_pool(config, pool_config).await?;
```

### Transactions

```rust
// Begin transaction
let tx = client.begin().await?;

// Execute queries within transaction
tx.execute("INSERT INTO users (name) VALUES ('Alice')").await?;
tx.execute("INSERT INTO users (name) VALUES ('Bob')").await?;

// Commit (or rollback on error)
tx.commit().await?;
```

### Query Builder

```rust
use aegis_client::query::QueryBuilder;

let query = QueryBuilder::select()
    .columns(&["id", "name", "email"])
    .from("users")
    .where_clause("age > ?", 21)
    .order_by("name", Order::Asc)
    .limit(10)
    .build();

let result = client.execute(query).await?;
```

### Key-Value Operations

```rust
// Set a key
client.kv_set("user:1", json!({"name": "Alice"})).await?;

// Get a key
let value = client.kv_get("user:1").await?;

// Delete a key
client.kv_delete("user:1").await?;

// List keys with prefix
let keys = client.kv_list("user:*").await?;
```

## Configuration

```rust
let config = ClientConfig::new("localhost:9090")
    .with_auth("username", "password")
    .with_timeout(Duration::from_secs(30))
    .with_retry_policy(RetryPolicy::exponential(3))
    .with_tls(true);
```

## Error Handling

```rust
use aegis_client::error::ClientError;

match client.query("SELECT * FROM users").await {
    Ok(result) => println!("Got {} rows", result.len()),
    Err(ClientError::ConnectionFailed(e)) => eprintln!("Connection error: {}", e),
    Err(ClientError::QueryFailed(e)) => eprintln!("Query error: {}", e),
    Err(ClientError::Timeout) => eprintln!("Request timed out"),
    Err(e) => eprintln!("Other error: {}", e),
}
```

## Tests

```bash
cargo test -p aegis-client
```

**Test count:** 45 tests

## License

Apache-2.0
