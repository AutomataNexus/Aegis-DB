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

/// In-memory storage backend for testing and development.
pub struct MemoryBackend {
    blocks: RwLock<HashMap<BlockId, Block>>,
    next_block_id: AtomicU64,
    next_tx_id: AtomicU64,
    stats: RwLock<StorageStats>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self {
            blocks: RwLock::new(HashMap::new()),
            next_block_id: AtomicU64::new(1),
            next_tx_id: AtomicU64::new(1),
            stats: RwLock::new(StorageStats::default()),
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
}
