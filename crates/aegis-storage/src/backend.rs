//! Aegis Backend - Storage Backend Abstraction
//!
//! Defines the pluggable storage backend trait and implementations for
//! local filesystem, memory, and distributed storage. Enables consistent
//! storage operations across different deployment models.
//!
//! Key Features:
//! - Async storage operations for non-blocking I/O
//! - Transaction support with begin/commit/rollback
//! - Block-level read/write/delete operations
//! - Replication hooks for distributed deployments
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::block::Block;
use aegis_common::{BlockId, Result, TransactionId, AegisError};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

// =============================================================================
// Storage Backend Trait
// =============================================================================

/// Pluggable storage backend interface.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Write a block to storage.
    async fn write_block(&self, block: Block) -> Result<BlockId>;

    /// Read a block from storage.
    async fn read_block(&self, id: BlockId) -> Result<Block>;

    /// Delete a block from storage.
    async fn delete_block(&self, id: BlockId) -> Result<()>;

    /// Check if a block exists.
    async fn block_exists(&self, id: BlockId) -> Result<bool>;

    /// Begin a new transaction.
    async fn begin_transaction(&self) -> Result<TransactionId>;

    /// Commit a transaction.
    async fn commit_transaction(&self, tx_id: TransactionId) -> Result<()>;

    /// Rollback a transaction.
    async fn rollback_transaction(&self, tx_id: TransactionId) -> Result<()>;

    /// Sync all pending writes to durable storage.
    async fn sync(&self) -> Result<()>;

    /// Get storage statistics.
    fn stats(&self) -> StorageStats;
}

// =============================================================================
// Storage Statistics
// =============================================================================

/// Statistics about storage usage and operations.
#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    pub total_blocks: u64,
    pub total_bytes: u64,
    pub read_ops: u64,
    pub write_ops: u64,
    pub delete_ops: u64,
}

// =============================================================================
// Memory Backend
// =============================================================================

/// Transaction state tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    Active,
    Committed,
    RolledBack,
}

/// In-memory storage backend for testing and development.
///
/// NOTE: Current transaction implementation is simplified.
/// Full ACID transactions require MVCC or WAL integration which is planned for v0.2.
pub struct MemoryBackend {
    blocks: RwLock<HashMap<BlockId, Block>>,
    next_block_id: AtomicU64,
    next_tx_id: AtomicU64,
    stats: RwLock<StorageStats>,
    /// Track active transactions and their state
    transactions: RwLock<HashMap<TransactionId, TransactionState>>,
    /// Pending writes per transaction (block_id -> block data)
    pending_writes: RwLock<HashMap<TransactionId, HashMap<BlockId, Block>>>,
    /// Pending deletes per transaction
    pending_deletes: RwLock<HashMap<TransactionId, Vec<BlockId>>>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self {
            blocks: RwLock::new(HashMap::new()),
            next_block_id: AtomicU64::new(1),
            next_tx_id: AtomicU64::new(1),
            stats: RwLock::new(StorageStats::default()),
            transactions: RwLock::new(HashMap::new()),
            pending_writes: RwLock::new(HashMap::new()),
            pending_deletes: RwLock::new(HashMap::new()),
        }
    }

    fn allocate_block_id(&self) -> BlockId {
        BlockId(self.next_block_id.fetch_add(1, Ordering::SeqCst))
    }

    fn allocate_tx_id(&self) -> TransactionId {
        TransactionId(self.next_tx_id.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for MemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageBackend for MemoryBackend {
    async fn write_block(&self, mut block: Block) -> Result<BlockId> {
        let block_id = if block.header.block_id.0 == 0 {
            self.allocate_block_id()
        } else {
            block.header.block_id
        };

        block.header.block_id = block_id;
        let size = block.data.len() as u64;

        let mut blocks = self.blocks.write();
        let is_new = !blocks.contains_key(&block_id);
        blocks.insert(block_id, block);

        let mut stats = self.stats.write();
        stats.write_ops += 1;
        if is_new {
            stats.total_blocks += 1;
            stats.total_bytes += size;
        }

        Ok(block_id)
    }

    async fn read_block(&self, id: BlockId) -> Result<Block> {
        let blocks = self.blocks.read();
        let block = blocks
            .get(&id)
            .cloned()
            .ok_or_else(|| AegisError::BlockNotFound(id.0))?;

        self.stats.write().read_ops += 1;
        Ok(block)
    }

    async fn delete_block(&self, id: BlockId) -> Result<()> {
        let mut blocks = self.blocks.write();
        if let Some(block) = blocks.remove(&id) {
            let mut stats = self.stats.write();
            stats.delete_ops += 1;
            stats.total_blocks -= 1;
            stats.total_bytes -= block.data.len() as u64;
            Ok(())
        } else {
            Err(AegisError::BlockNotFound(id.0))
        }
    }

    async fn block_exists(&self, id: BlockId) -> Result<bool> {
        Ok(self.blocks.read().contains_key(&id))
    }

    async fn begin_transaction(&self) -> Result<TransactionId> {
        let tx_id = self.allocate_tx_id();
        self.transactions.write().insert(tx_id, TransactionState::Active);
        self.pending_writes.write().insert(tx_id, HashMap::new());
        self.pending_deletes.write().insert(tx_id, Vec::new());
        Ok(tx_id)
    }

    async fn commit_transaction(&self, tx_id: TransactionId) -> Result<()> {
        // Verify transaction is active
        {
            let transactions = self.transactions.read();
            match transactions.get(&tx_id) {
                Some(TransactionState::Active) => {}
                Some(TransactionState::Committed) => {
                    return Err(AegisError::Transaction("Transaction already committed".to_string()));
                }
                Some(TransactionState::RolledBack) => {
                    return Err(AegisError::Transaction("Transaction was rolled back".to_string()));
                }
                None => {
                    return Err(AegisError::Transaction("Transaction not found".to_string()));
                }
            }
        }

        // Apply pending writes
        let pending = self.pending_writes.write().remove(&tx_id).unwrap_or_default();
        {
            let mut blocks = self.blocks.write();
            let mut stats = self.stats.write();
            for (block_id, block) in pending {
                let size = block.data.len() as u64;
                let is_new = !blocks.contains_key(&block_id);
                blocks.insert(block_id, block);
                if is_new {
                    stats.total_blocks += 1;
                    stats.total_bytes += size;
                }
            }
        }

        // Apply pending deletes
        let deletes = self.pending_deletes.write().remove(&tx_id).unwrap_or_default();
        {
            let mut blocks = self.blocks.write();
            let mut stats = self.stats.write();
            for block_id in deletes {
                if let Some(block) = blocks.remove(&block_id) {
                    stats.total_blocks -= 1;
                    stats.total_bytes -= block.data.len() as u64;
                }
            }
        }

        // Mark transaction as committed
        self.transactions.write().insert(tx_id, TransactionState::Committed);
        Ok(())
    }

    async fn rollback_transaction(&self, tx_id: TransactionId) -> Result<()> {
        // Verify transaction exists
        {
            let transactions = self.transactions.read();
            match transactions.get(&tx_id) {
                Some(TransactionState::Active) => {}
                Some(TransactionState::Committed) => {
                    return Err(AegisError::Transaction("Cannot rollback committed transaction".to_string()));
                }
                Some(TransactionState::RolledBack) => {
                    return Ok(()); // Already rolled back, idempotent
                }
                None => {
                    return Err(AegisError::Transaction("Transaction not found".to_string()));
                }
            }
        }

        // Discard pending writes and deletes
        self.pending_writes.write().remove(&tx_id);
        self.pending_deletes.write().remove(&tx_id);

        // Mark transaction as rolled back
        self.transactions.write().insert(tx_id, TransactionState::RolledBack);
        Ok(())
    }

    async fn sync(&self) -> Result<()> {
        Ok(())
    }

    fn stats(&self) -> StorageStats {
        self.stats.read().clone()
    }
}

// =============================================================================
// Local Filesystem Backend
// =============================================================================

/// Local filesystem storage backend.
pub struct LocalBackend {
    data_dir: PathBuf,
    blocks: RwLock<HashMap<BlockId, PathBuf>>,
    next_block_id: AtomicU64,
    next_tx_id: AtomicU64,
    stats: RwLock<StorageStats>,
    sync_writes: bool,
}

impl LocalBackend {
    pub fn new(data_dir: PathBuf, sync_writes: bool) -> Result<Self> {
        std::fs::create_dir_all(&data_dir)?;

        let backend = Self {
            data_dir,
            blocks: RwLock::new(HashMap::new()),
            next_block_id: AtomicU64::new(1),
            next_tx_id: AtomicU64::new(1),
            stats: RwLock::new(StorageStats::default()),
            sync_writes,
        };

        Ok(backend)
    }

    fn block_path(&self, id: BlockId) -> PathBuf {
        self.data_dir.join(format!("block_{:016x}.dat", id.0))
    }

    fn allocate_block_id(&self) -> BlockId {
        BlockId(self.next_block_id.fetch_add(1, Ordering::SeqCst))
    }

    fn allocate_tx_id(&self) -> TransactionId {
        TransactionId(self.next_tx_id.fetch_add(1, Ordering::SeqCst))
    }
}

#[async_trait]
impl StorageBackend for LocalBackend {
    async fn write_block(&self, mut block: Block) -> Result<BlockId> {
        let block_id = if block.header.block_id.0 == 0 {
            self.allocate_block_id()
        } else {
            block.header.block_id
        };

        block.header.block_id = block_id;
        let path = self.block_path(block_id);
        let data = block.to_bytes()?;

        tokio::fs::write(&path, &data).await?;

        if self.sync_writes {
            let file = tokio::fs::File::open(&path).await?;
            file.sync_all().await?;
        }

        let mut blocks = self.blocks.write();
        let is_new = !blocks.contains_key(&block_id);
        blocks.insert(block_id, path);

        let mut stats = self.stats.write();
        stats.write_ops += 1;
        if is_new {
            stats.total_blocks += 1;
            stats.total_bytes += data.len() as u64;
        }

        Ok(block_id)
    }

    async fn read_block(&self, id: BlockId) -> Result<Block> {
        let path = self.block_path(id);
        let data = tokio::fs::read(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AegisError::BlockNotFound(id.0)
            } else {
                AegisError::Io(e)
            }
        })?;

        self.stats.write().read_ops += 1;
        Block::from_bytes(&data)
    }

    async fn delete_block(&self, id: BlockId) -> Result<()> {
        let path = self.block_path(id);
        tokio::fs::remove_file(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AegisError::BlockNotFound(id.0)
            } else {
                AegisError::Io(e)
            }
        })?;

        self.blocks.write().remove(&id);
        let mut stats = self.stats.write();
        stats.delete_ops += 1;
        stats.total_blocks = stats.total_blocks.saturating_sub(1);

        Ok(())
    }

    async fn block_exists(&self, id: BlockId) -> Result<bool> {
        let path = self.block_path(id);
        Ok(tokio::fs::try_exists(&path).await.unwrap_or(false))
    }

    async fn begin_transaction(&self) -> Result<TransactionId> {
        Ok(self.allocate_tx_id())
    }

    async fn commit_transaction(&self, _tx_id: TransactionId) -> Result<()> {
        Ok(())
    }

    async fn rollback_transaction(&self, _tx_id: TransactionId) -> Result<()> {
        Ok(())
    }

    async fn sync(&self) -> Result<()> {
        Ok(())
    }

    fn stats(&self) -> StorageStats {
        self.stats.read().clone()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use aegis_common::BlockType;
    use bytes::Bytes;

    #[tokio::test]
    async fn test_memory_backend_write_read() {
        let backend = MemoryBackend::new();
        let data = Bytes::from("test data");
        let block = Block::new(BlockId(0), BlockType::TableData, data.clone());

        let id = backend.write_block(block).await.unwrap();
        let read_block = backend.read_block(id).await.unwrap();

        assert_eq!(read_block.data, data);
    }

    #[tokio::test]
    async fn test_memory_backend_delete() {
        let backend = MemoryBackend::new();
        let block = Block::new(BlockId(0), BlockType::TableData, Bytes::from("test"));

        let id = backend.write_block(block).await.unwrap();
        assert!(backend.block_exists(id).await.unwrap());

        backend.delete_block(id).await.unwrap();
        assert!(!backend.block_exists(id).await.unwrap());
    }

    #[tokio::test]
    async fn test_local_backend_write_read() {
        let temp_dir = tempfile::tempdir().unwrap();
        let backend = LocalBackend::new(temp_dir.path().to_path_buf(), false).unwrap();

        let data = Bytes::from("test data");
        let block = Block::new(BlockId(0), BlockType::TableData, data.clone());

        let id = backend.write_block(block).await.unwrap();
        let read_block = backend.read_block(id).await.unwrap();

        assert_eq!(read_block.data, data);
    }

    #[tokio::test]
    async fn test_transaction_commit_rollback_state() {
        let backend = MemoryBackend::new();

        // Begin transaction
        let tx_id = backend.begin_transaction().await.unwrap();

        // Commit should succeed
        backend.commit_transaction(tx_id).await.unwrap();

        // Second commit should fail (already committed)
        let result = backend.commit_transaction(tx_id).await;
        assert!(result.is_err());

        // Rollback should fail (already committed)
        let result = backend.rollback_transaction(tx_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let backend = MemoryBackend::new();

        // Begin transaction
        let tx_id = backend.begin_transaction().await.unwrap();

        // Rollback should succeed
        backend.rollback_transaction(tx_id).await.unwrap();

        // Second rollback should be idempotent (succeed)
        backend.rollback_transaction(tx_id).await.unwrap();

        // Commit should fail (already rolled back)
        let result = backend.commit_transaction(tx_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_transaction_not_found() {
        let backend = MemoryBackend::new();

        // Try to commit a non-existent transaction
        let fake_tx_id = TransactionId(999);
        let result = backend.commit_transaction(fake_tx_id).await;
        assert!(result.is_err());

        // Try to rollback a non-existent transaction
        let result = backend.rollback_transaction(fake_tx_id).await;
        assert!(result.is_err());
    }
}
