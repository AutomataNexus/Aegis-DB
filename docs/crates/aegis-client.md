---
layout: default
title: aegis-client
parent: Crate Documentation
nav_order: 9
description: "Client SDK"
---

# aegis-client

Native Rust Client SDK for Aegis Database Platform.

## Overview

Async-first client library for connecting to Aegis database instances.
Provides connection pooling, query building, and transaction management.

## Modules

### config.rs
Client configuration:
- `ClientConfig` - Complete client configuration with connection, pool, retry, timeout settings
- `ConnectionConfig` - Host, port, database, credentials, SSL mode
- `PoolConfig` - Min/max connections, acquire/idle timeout, test on acquire
- `RetryConfig` - Max retries, exponential backoff with initial/max delay
- `TimeoutConfig` - Connect, query, statement timeouts
- `SslMode` - Disable, Prefer, Require, VerifyCa, VerifyFull
- URL parsing (aegis://user:pass@host:port/database)

### connection.rs
Database connection handling:
- `Connection` - Database connection with query execution
- `ConnectionStats` - Connection metrics (age, idle time, queries executed)
- `PooledConnection` - Connection wrapper for pool management
- Transaction support (begin, commit, rollback)
- Query and execute methods with parameter binding

### pool.rs
Connection pool management:
- `ConnectionPool` - Thread-safe connection pool with semaphore-based limiting
- `PoolStats` - Pool metrics (created, acquired, released, utilization)
- Automatic connection creation and recycling
- Health checking and stale connection removal
- Configurable min/max connections

### query.rs
Type-safe query building:
- `Query` - Prepared query with SQL and parameters
- `QueryBuilder` - Fluent API for building queries
- SELECT, INSERT, UPDATE, DELETE support
- WHERE conditions (eq, ne, gt, gte, lt, lte, like, in, null)
- JOIN support (inner, left, right, full)
- ORDER BY, GROUP BY, HAVING
- LIMIT and OFFSET
- Parameterized queries with placeholders

### result.rs
Query result handling:
- `Value` - Database value (Null, Bool, Int, Float, String, Bytes, Array, Object, Timestamp)
- `Column` - Column metadata (name, type, nullable)
- `DataType` - SQL data types
- `Row` - Result row with column access by index or name
- `QueryResult` - Complete result set with columns, rows, affected count

### transaction.rs
Transaction management:
- `Transaction` - Database transaction with commit/rollback
- `Savepoint` - Nested savepoints within transactions
- `TransactionOptions` - Isolation level, read-only, deferrable
- `IsolationLevel` - ReadUncommitted, ReadCommitted, RepeatableRead, Serializable
- Automatic rollback on drop

### error.rs
Error types:
- `ClientError` - All client operation errors
- Connection errors (failed, timeout, closed)
- Query and transaction errors
- Pool errors (exhausted, timeout)
- Retryable error detection

## Usage Example

```rust
use aegis_client::*;

// Connect with URL
let client = AegisClient::connect("aegis://localhost:5432/mydb").await?;

// Simple query
let result = client.query("SELECT * FROM users").await?;
for row in result.iter() {
    let id: i64 = row.get_int(0).unwrap();
    let name: &str = row.get_str(1).unwrap();
    println!("{}: {}", id, name);
}

// Query with parameters
let result = client.query_with_params(
    "SELECT * FROM users WHERE id = $1",
    vec![Value::Int(1)],
).await?;

// Execute statement
let affected = client.execute("DELETE FROM users WHERE inactive = true").await?;
println!("Deleted {} rows", affected);
```

## Query Builder Example

```rust
use aegis_client::*;

// SELECT query
let query = QueryBuilder::new()
    .select(&["id", "name", "email"])
    .from("users")
    .where_eq("status", "active")
    .where_gt("age", 18)
    .order_by_desc("created_at")
    .limit(10)
    .build();

// INSERT query
let query = QueryBuilder::new()
    .insert_into("users")
    .columns(&["name", "email"])
    .values(vec![
        Value::String("Alice".to_string()),
        Value::String("alice@example.com".to_string()),
    ])
    .build();

// UPDATE query
let query = QueryBuilder::new()
    .update("users")
    .set("name", "Bob")
    .set("updated_at", Value::Timestamp(now))
    .where_eq("id", 1)
    .build();

// DELETE query
let query = QueryBuilder::new()
    .delete_from("users")
    .where_eq("id", 1)
    .build();
```

## Transaction Example

```rust
use aegis_client::*;

// Start transaction
let tx = client.begin().await?;

// Execute within transaction
tx.execute("INSERT INTO accounts (id, balance) VALUES (1, 1000)").await?;
tx.execute("INSERT INTO accounts (id, balance) VALUES (2, 500)").await?;

// Transfer funds
tx.execute("UPDATE accounts SET balance = balance - 100 WHERE id = 1").await?;
tx.execute("UPDATE accounts SET balance = balance + 100 WHERE id = 2").await?;

// Commit
tx.commit().await?;

// Or with savepoints
let tx = client.begin().await?;
tx.execute("INSERT INTO users (name) VALUES ('Alice')").await?;

let sp = tx.savepoint("before_bob").await?;
tx.execute("INSERT INTO users (name) VALUES ('Bob')").await?;

// Rollback to savepoint if needed
sp.rollback().await?;

tx.commit().await?;
```

## Configuration Example

```rust
use aegis_client::*;
use std::time::Duration;

let config = ClientConfig::new("db.example.com", 5432, "mydb")
    .with_pool_size(5, 20)
    .with_connect_timeout(Duration::from_secs(5))
    .with_query_timeout(Duration::from_secs(30));

let client = AegisClient::new(config).await?;

// Check pool stats
let stats = client.pool_stats();
println!("Pool utilization: {:.1}%", stats.utilization());
```

## Tests

45 tests covering all modules:
- Configuration parsing and URL handling
- Connection creation and management
- Connection pooling and statistics
- Query builder for all SQL operations
- Result set handling and value types
- Transaction and savepoint management
- Error types and retryable detection
