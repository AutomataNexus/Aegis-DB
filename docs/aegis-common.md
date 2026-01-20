# aegis-common

Shared types, error definitions, and utilities used across all Aegis crates.

## Overview

The `aegis-common` crate provides the foundational types and utilities that are shared across the entire Aegis database platform. This includes error handling, core data types, configuration structures, and common utilities.

## Modules

### error.rs
Unified error handling with the `AegisError` enum:
- `Storage` - Storage layer errors
- `Transaction` - Transaction-related errors
- `Query` - Query parsing/execution errors
- `Parse` - SQL parsing errors
- `Replication` - Distributed replication errors
- `Auth` - Authentication/authorization errors
- `Config` - Configuration errors
- `Internal` - Internal system errors

Error classification methods:
- `is_retryable()` - Whether the operation can be retried
- `is_user_error()` - Whether the error is due to user input

### types.rs
Core data types:

**Identifiers:**
- `BlockId` - Storage block identifier
- `PageId` - Buffer pool page identifier
- `TransactionId` - Transaction identifier
- `NodeId` - Cluster node identifier
- `ShardId` - Shard identifier
- `Lsn` - Log sequence number

**Value Types:**
- `Value` enum - Multi-paradigm value representation (Null, Boolean, Integer, Float, String, Bytes, Timestamp, Array, Object)
- `DataType` enum - SQL data types (Boolean, Integer, Float, Text, Timestamp, etc.)

**Structures:**
- `Row` - A row of values
- `ColumnDef` - Column metadata
- `TimeRange` - Time range for queries
- `KeyRange` - Key range for sharding

### config.rs
Configuration management with TOML support.

### utils.rs
Common utility functions.

## Usage

```rust
use aegis_common::{AegisError, Result, Value, DataType, TransactionId};

fn example() -> Result<Value> {
    let value = Value::Integer(42);
    Ok(value)
}
```

## Tests

4 tests covering error handling and type conversions.
