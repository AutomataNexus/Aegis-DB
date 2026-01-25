<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/assets/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-storage

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.0-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

High-performance storage engine for the Aegis Database Platform.

## Overview

`aegis-storage` provides the core storage layer with pluggable backends, write-ahead logging, MVCC transactions, and block-level compression. It serves as the foundation for all data persistence in Aegis.

## Features

- **Pluggable Backends** - Memory and local filesystem backends
- **Write-Ahead Logging (WAL)** - Durability and crash recovery
- **MVCC Transactions** - Snapshot isolation with multi-version concurrency control
- **Block Compression** - LZ4 and Zstd compression support
- **Buffer Pool** - LRU-based page caching for optimal I/O

## Architecture

```
┌─────────────────────────────────────┐
│          Storage API                │
├─────────────────────────────────────┤
│     Transaction Manager (MVCC)      │
├─────────────────────────────────────┤
│         Buffer Pool (LRU)           │
├─────────────────────────────────────┤
│      Write-Ahead Log (WAL)          │
├─────────────┬───────────────────────┤
│   Memory    │    Local FS Backend   │
│   Backend   │                       │
└─────────────┴───────────────────────┘
```

## Modules

| Module | Description |
|--------|-------------|
| `backend` | Storage backend trait and implementations |
| `block` | Block structure and serialization |
| `buffer` | Buffer pool with LRU eviction |
| `page` | Page management and allocation |
| `transaction` | MVCC transaction handling |
| `wal` | Write-ahead logging for durability |

## Usage

```toml
[dependencies]
aegis-storage = { path = "../aegis-storage" }
```

### Example

```rust
use aegis_storage::{StorageEngine, StorageConfig};
use aegis_storage::backend::MemoryBackend;

// Create storage engine with memory backend
let config = StorageConfig::default();
let engine = StorageEngine::new(config)?;

// Begin a transaction
let tx = engine.begin_transaction()?;

// Write data
engine.put(&tx, b"key", b"value")?;

// Commit
engine.commit(tx)?;

// Read data
let value = engine.get(b"key")?;
```

### Compression

```rust
use aegis_storage::block::{Block, CompressionType};

// Create compressed block
let block = Block::new(data)
    .with_compression(CompressionType::Lz4);

// Compression is automatic on write
engine.write_block(block)?;
```

## Configuration

```toml
[storage]
backend = "local"              # "memory" or "local"
data_directory = "/var/aegis"
compression = "lz4"            # "none", "lz4", "zstd"
buffer_pool_size = "1GB"
wal_enabled = true
sync_on_commit = true
```

## Performance

- **Write throughput**: ~500K ops/sec (memory backend)
- **Read throughput**: ~1M ops/sec (cached)
- **Compression ratio**: 2-10x depending on data

## Tests

```bash
cargo test -p aegis-storage
```

**Test count:** 23 tests

## License

Apache-2.0
