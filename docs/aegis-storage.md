# aegis-storage

Storage engine providing persistence, transactions, and durability guarantees.

## Overview

The `aegis-storage` crate implements the core storage layer for the Aegis database. It provides pluggable storage backends, buffer pool management, write-ahead logging, and MVCC-based transaction support.

## Modules

### backend.rs
Pluggable storage backend abstraction:

**StorageBackend Trait:**
```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn read(&self, block_id: BlockId) -> Result<Option<Block>>;
    async fn write(&self, block_id: BlockId, block: Block) -> Result<()>;
    async fn delete(&self, block_id: BlockId) -> Result<()>;
    async fn exists(&self, block_id: BlockId) -> Result<bool>;
    async fn sync(&self) -> Result<()>;
}
```

**Implementations:**
- `MemoryBackend` - In-memory storage for testing
- `LocalBackend` - Local filesystem storage with configurable data directory

### block.rs
Block-level storage with compression:

- `Block` struct with header, data, and checksum
- `BlockHeader` with metadata (id, type, compression, size)
- Compression support: None, LZ4, Zstd, Snappy
- CRC32 checksum validation

### page.rs
Buffer pool page management:

- 8KB page size (configurable)
- Slot-based tuple storage
- Page types: Data, Index, Overflow, Free
- Pin counting for concurrent access
- Dirty flag tracking

### buffer.rs
Buffer pool with LRU eviction:

- `BufferPool` - Main buffer management
- `PageHandle` - RAII wrapper for page access
- LRU eviction policy
- Configurable pool size

### wal.rs
Write-ahead logging for durability:

**Log Record Types:**
- `Begin` - Transaction start
- `Commit` - Transaction commit
- `Abort` - Transaction abort
- `Insert` - Row insertion
- `Update` - Row update
- `Delete` - Row deletion

Features:
- LSN-based ordering
- Checksum validation
- Append-only log structure

### transaction.rs
MVCC transaction system:

**Components:**
- `Transaction` - Transaction state and operations
- `TransactionManager` - Transaction lifecycle management
- `LockManager` - Shared/exclusive lock management
- `Snapshot` - Point-in-time database view
- `Version` - Row version with visibility info

**Transaction States:**
- Active, Committed, Aborted

**Isolation:**
- Snapshot isolation via MVCC
- Visibility checks based on transaction timestamps

## Usage

```rust
use aegis_storage::{StorageBackend, MemoryBackend, TransactionManager};

// Create storage backend
let backend = MemoryBackend::new();

// Create transaction manager
let tx_manager = TransactionManager::new();

// Begin transaction
let tx = tx_manager.begin()?;

// Perform operations...

// Commit
tx_manager.commit(tx.id)?;
```

## Tests

23 tests covering storage backends, buffer pool, WAL, and transactions.
