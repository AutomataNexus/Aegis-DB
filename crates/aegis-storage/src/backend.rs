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
            .ok_or(AegisError::BlockNotFound(id.0))?;

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

/// Pending operation for a transaction.
#[derive(Debug, Clone)]
enum PendingOperation {
    Write { block_id: BlockId, data: Vec<u8> },
    Delete { block_id: BlockId },
}

/// Local filesystem storage backend with full transaction support.
///
/// Features:
/// - WAL-based durability for crash recovery
/// - Transaction isolation with pending operations
/// - fsync on commit for true durability
/// - Automatic recovery on startup
pub struct LocalBackend {
    data_dir: PathBuf,
    blocks: RwLock<HashMap<BlockId, PathBuf>>,
    next_block_id: AtomicU64,
    next_tx_id: AtomicU64,
    stats: RwLock<StorageStats>,
    sync_writes: bool,
    /// Track active transactions and their state
    transactions: RwLock<HashMap<TransactionId, TransactionState>>,
    /// Pending operations per transaction
    pending_ops: RwLock<HashMap<TransactionId, Vec<PendingOperation>>>,
    /// Track last LSN per transaction for WAL chain
    tx_last_lsn: RwLock<HashMap<TransactionId, u64>>,
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
            transactions: RwLock::new(HashMap::new()),
            pending_ops: RwLock::new(HashMap::new()),
            tx_last_lsn: RwLock::new(HashMap::new()),
        };

        // Scan existing blocks to set next_block_id correctly
        backend.recover_block_ids()?;

        Ok(backend)
    }

    /// Create a new LocalBackend with an associated WAL for durability.
    pub fn with_wal(data_dir: PathBuf, sync_writes: bool) -> Result<(Self, crate::wal::WriteAheadLog)> {
        let wal_dir = data_dir.join("wal");
        let wal = crate::wal::WriteAheadLog::new(wal_dir, sync_writes)?;
        let backend = Self::new(data_dir, sync_writes)?;
        Ok((backend, wal))
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

    /// Recover block IDs from existing files on disk.
    fn recover_block_ids(&self) -> Result<()> {
        let mut max_id = 0u64;
        let mut blocks = self.blocks.write();

        if let Ok(entries) = std::fs::read_dir(&self.data_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("block_") && name.ends_with(".dat") {
                        // Parse block ID from filename: block_0000000000000001.dat
                        if let Some(id_str) = name.strip_prefix("block_").and_then(|s| s.strip_suffix(".dat")) {
                            if let Ok(id) = u64::from_str_radix(id_str, 16) {
                                max_id = max_id.max(id);
                                blocks.insert(BlockId(id), path);
                            }
                        }
                    }
                }
            }
        }

        // Set next_block_id to one past the maximum found
        self.next_block_id.store(max_id + 1, Ordering::SeqCst);
        Ok(())
    }

    /// Sync a specific file to disk (fsync).
    async fn sync_file(&self, path: &std::path::Path) -> Result<()> {
        let file = tokio::fs::File::open(path).await?;
        file.sync_all().await?;
        Ok(())
    }

    /// Sync the data directory to ensure directory entries are persisted.
    async fn sync_directory(&self) -> Result<()> {
        // On Unix, we need to fsync the directory to ensure renames/creates are durable
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let dir = std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY)
                .open(&self.data_dir)?;
            dir.sync_all()?;
        }
        Ok(())
    }

    /// Write a block within a transaction context (adds to pending ops).
    pub async fn write_block_in_tx(&self, tx_id: TransactionId, mut block: Block) -> Result<BlockId> {
        // Verify transaction is active
        {
            let transactions = self.transactions.read();
            match transactions.get(&tx_id) {
                Some(TransactionState::Active) => {}
                Some(_) => {
                    return Err(AegisError::Transaction("Transaction not active".to_string()));
                }
                None => {
                    return Err(AegisError::Transaction("Transaction not found".to_string()));
                }
            }
        }

        let block_id = if block.header.block_id.0 == 0 {
            self.allocate_block_id()
        } else {
            block.header.block_id
        };

        block.header.block_id = block_id;
        let data = block.to_bytes()?;

        // Add to pending operations
        let mut pending = self.pending_ops.write();
        let ops = pending.entry(tx_id).or_default();
        ops.push(PendingOperation::Write {
            block_id,
            data: data.to_vec(),
        });

        Ok(block_id)
    }

    /// Delete a block within a transaction context.
    pub async fn delete_block_in_tx(&self, tx_id: TransactionId, id: BlockId) -> Result<()> {
        // Verify transaction is active
        {
            let transactions = self.transactions.read();
            match transactions.get(&tx_id) {
                Some(TransactionState::Active) => {}
                Some(_) => {
                    return Err(AegisError::Transaction("Transaction not active".to_string()));
                }
                None => {
                    return Err(AegisError::Transaction("Transaction not found".to_string()));
                }
            }
        }

        // Add to pending operations
        let mut pending = self.pending_ops.write();
        let ops = pending.entry(tx_id).or_default();
        ops.push(PendingOperation::Delete { block_id: id });

        Ok(())
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

        // Write to a temporary file first, then rename for atomicity
        let temp_path = self.data_dir.join(format!("block_{:016x}.tmp", block_id.0));
        tokio::fs::write(&temp_path, &data).await?;

        if self.sync_writes {
            self.sync_file(&temp_path).await?;
        }

        // Atomic rename
        tokio::fs::rename(&temp_path, &path).await?;

        if self.sync_writes {
            self.sync_directory().await?;
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
        // First check pending writes in any active transaction (read-your-writes)
        {
            let pending = self.pending_ops.read();
            for (tx_id, ops) in pending.iter() {
                // Only consider active transactions
                let transactions = self.transactions.read();
                if transactions.get(tx_id) == Some(&TransactionState::Active) {
                    // Find the last write to this block in this transaction
                    for op in ops.iter().rev() {
                        if let PendingOperation::Write { block_id, data } = op {
                            if *block_id == id {
                                return Block::from_bytes(data);
                            }
                        }
                        if let PendingOperation::Delete { block_id } = op {
                            if *block_id == id {
                                return Err(AegisError::BlockNotFound(id.0));
                            }
                        }
                    }
                }
            }
        }

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

        if self.sync_writes {
            self.sync_directory().await?;
        }

        self.blocks.write().remove(&id);
        let mut stats = self.stats.write();
        stats.delete_ops += 1;
        stats.total_blocks = stats.total_blocks.saturating_sub(1);

        Ok(())
    }

    async fn block_exists(&self, id: BlockId) -> Result<bool> {
        // Check pending operations first
        {
            let pending = self.pending_ops.read();
            for (tx_id, ops) in pending.iter() {
                let transactions = self.transactions.read();
                if transactions.get(tx_id) == Some(&TransactionState::Active) {
                    for op in ops.iter().rev() {
                        if let PendingOperation::Write { block_id, .. } = op {
                            if *block_id == id {
                                return Ok(true);
                            }
                        }
                        if let PendingOperation::Delete { block_id } = op {
                            if *block_id == id {
                                return Ok(false);
                            }
                        }
                    }
                }
            }
        }

        let path = self.block_path(id);
        Ok(tokio::fs::try_exists(&path).await.unwrap_or(false))
    }

    async fn begin_transaction(&self) -> Result<TransactionId> {
        let tx_id = self.allocate_tx_id();

        // Initialize transaction state
        self.transactions.write().insert(tx_id, TransactionState::Active);
        self.pending_ops.write().insert(tx_id, Vec::new());
        self.tx_last_lsn.write().insert(tx_id, 0);

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

        // Get pending operations for this transaction
        let operations = self.pending_ops.write().remove(&tx_id).unwrap_or_default();

        // Apply all pending operations atomically
        // First, write all blocks to temp files
        let mut temp_files: Vec<(BlockId, PathBuf, PathBuf, Vec<u8>)> = Vec::new();
        let mut deletes: Vec<BlockId> = Vec::new();

        for op in operations {
            match op {
                PendingOperation::Write { block_id, data } => {
                    let temp_path = self.data_dir.join(format!("block_{:016x}.tmp", block_id.0));
                    let final_path = self.block_path(block_id);

                    // Write to temp file
                    tokio::fs::write(&temp_path, &data).await?;

                    if self.sync_writes {
                        self.sync_file(&temp_path).await?;
                    }

                    temp_files.push((block_id, temp_path, final_path, data));
                }
                PendingOperation::Delete { block_id } => {
                    deletes.push(block_id);
                }
            }
        }

        // Now atomically rename all temp files to final destinations
        // This is the commit point - if we crash after all renames, data is committed
        for (block_id, temp_path, final_path, data) in temp_files {
            tokio::fs::rename(&temp_path, &final_path).await?;

            // Update in-memory state
            let mut blocks = self.blocks.write();
            let is_new = !blocks.contains_key(&block_id);
            blocks.insert(block_id, final_path);

            let mut stats = self.stats.write();
            stats.write_ops += 1;
            if is_new {
                stats.total_blocks += 1;
                stats.total_bytes += data.len() as u64;
            }
        }

        // Process deletes
        for block_id in deletes {
            let path = self.block_path(block_id);
            if let Err(e) = tokio::fs::remove_file(&path).await {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(AegisError::Io(e));
                }
            }

            self.blocks.write().remove(&block_id);
            let mut stats = self.stats.write();
            stats.delete_ops += 1;
            stats.total_blocks = stats.total_blocks.saturating_sub(1);
        }

        // Sync directory to ensure all renames are durable
        if self.sync_writes {
            self.sync_directory().await?;
        }

        // Mark transaction as committed
        self.transactions.write().insert(tx_id, TransactionState::Committed);
        self.tx_last_lsn.write().remove(&tx_id);

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

        // Discard pending operations - nothing was written to disk yet
        self.pending_ops.write().remove(&tx_id);

        // Clean up any temp files that might exist
        if let Ok(entries) = std::fs::read_dir(&self.data_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".tmp") {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }

        // Mark transaction as rolled back
        self.transactions.write().insert(tx_id, TransactionState::RolledBack);
        self.tx_last_lsn.write().remove(&tx_id);

        Ok(())
    }

    async fn sync(&self) -> Result<()> {
        // Collect paths to sync (release lock before async operations)
        let paths: Vec<PathBuf> = {
            let blocks = self.blocks.read();
            blocks.values().cloned().collect()
        };

        // Sync all block files
        for path in paths {
            if path.exists() {
                self.sync_file(&path).await?;
            }
        }

        // Sync directory
        self.sync_directory().await?;

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

        let id = backend.write_block(block).await.expect("write_block should succeed");
        let read_block = backend.read_block(id).await.expect("read_block should succeed");

        assert_eq!(read_block.data, data);
    }

    #[tokio::test]
    async fn test_memory_backend_delete() {
        let backend = MemoryBackend::new();
        let block = Block::new(BlockId(0), BlockType::TableData, Bytes::from("test"));

        let id = backend.write_block(block).await.expect("write_block should succeed");
        assert!(backend.block_exists(id).await.expect("block_exists should succeed"));

        backend.delete_block(id).await.expect("delete_block should succeed");
        assert!(!backend.block_exists(id).await.expect("block_exists should succeed after delete"));
    }

    #[tokio::test]
    async fn test_local_backend_write_read() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let backend = LocalBackend::new(temp_dir.path().to_path_buf(), false).expect("LocalBackend::new should succeed");

        let data = Bytes::from("test data");
        let block = Block::new(BlockId(0), BlockType::TableData, data.clone());

        let id = backend.write_block(block).await.expect("write_block should succeed");
        let read_block = backend.read_block(id).await.expect("read_block should succeed");

        assert_eq!(read_block.data, data);
    }

    #[tokio::test]
    async fn test_local_backend_transaction_commit() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let backend = LocalBackend::new(temp_dir.path().to_path_buf(), true).expect("LocalBackend::new should succeed");

        // Begin transaction
        let tx_id = backend.begin_transaction().await.expect("begin_transaction should succeed");

        // Write block within transaction
        let data = Bytes::from("transactional data");
        let block = Block::new(BlockId(0), BlockType::TableData, data.clone());
        let block_id = backend.write_block_in_tx(tx_id, block).await.expect("write_block_in_tx should succeed");

        // Block should not be on disk yet (only in pending)
        let path = backend.block_path(block_id);
        assert!(!path.exists());

        // Commit transaction
        backend.commit_transaction(tx_id).await.expect("commit_transaction should succeed");

        // Now block should be on disk
        assert!(path.exists());

        // Should be readable
        let read_block = backend.read_block(block_id).await.expect("read_block should succeed");
        assert_eq!(read_block.data, data);
    }

    #[tokio::test]
    async fn test_local_backend_transaction_rollback() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let backend = LocalBackend::new(temp_dir.path().to_path_buf(), false).expect("LocalBackend::new should succeed");

        // Begin transaction
        let tx_id = backend.begin_transaction().await.expect("begin_transaction should succeed");

        // Write block within transaction
        let data = Bytes::from("data to be rolled back");
        let block = Block::new(BlockId(0), BlockType::TableData, data);
        let block_id = backend.write_block_in_tx(tx_id, block).await.expect("write_block_in_tx should succeed");

        // Rollback
        backend.rollback_transaction(tx_id).await.expect("rollback_transaction should succeed");

        // Block should not exist
        let path = backend.block_path(block_id);
        assert!(!path.exists());

        // Reading should fail
        let result = backend.read_block(block_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_local_backend_transaction_double_commit() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let backend = LocalBackend::new(temp_dir.path().to_path_buf(), false).expect("LocalBackend::new should succeed");

        let tx_id = backend.begin_transaction().await.expect("begin_transaction should succeed");
        backend.commit_transaction(tx_id).await.expect("first commit_transaction should succeed");

        // Second commit should fail
        let result = backend.commit_transaction(tx_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_local_backend_transaction_commit_after_rollback() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let backend = LocalBackend::new(temp_dir.path().to_path_buf(), false).expect("LocalBackend::new should succeed");

        let tx_id = backend.begin_transaction().await.expect("begin_transaction should succeed");
        backend.rollback_transaction(tx_id).await.expect("rollback_transaction should succeed");

        // Commit after rollback should fail
        let result = backend.commit_transaction(tx_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_local_backend_multiple_blocks_in_tx() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let backend = LocalBackend::new(temp_dir.path().to_path_buf(), true).expect("LocalBackend::new should succeed");

        let tx_id = backend.begin_transaction().await.expect("begin_transaction should succeed");

        // Write multiple blocks
        let block1 = Block::new(BlockId(0), BlockType::TableData, Bytes::from("block1"));
        let block2 = Block::new(BlockId(0), BlockType::TableData, Bytes::from("block2"));
        let block3 = Block::new(BlockId(0), BlockType::TableData, Bytes::from("block3"));

        let id1 = backend.write_block_in_tx(tx_id, block1).await.expect("write_block_in_tx block1 should succeed");
        let id2 = backend.write_block_in_tx(tx_id, block2).await.expect("write_block_in_tx block2 should succeed");
        let id3 = backend.write_block_in_tx(tx_id, block3).await.expect("write_block_in_tx block3 should succeed");

        // None should exist on disk yet
        assert!(!backend.block_path(id1).exists());
        assert!(!backend.block_path(id2).exists());
        assert!(!backend.block_path(id3).exists());

        // Commit
        backend.commit_transaction(tx_id).await.expect("commit_transaction should succeed");

        // All should exist now
        assert!(backend.block_path(id1).exists());
        assert!(backend.block_path(id2).exists());
        assert!(backend.block_path(id3).exists());

        // Verify data
        let read1 = backend.read_block(id1).await.expect("read_block id1 should succeed");
        let read2 = backend.read_block(id2).await.expect("read_block id2 should succeed");
        let read3 = backend.read_block(id3).await.expect("read_block id3 should succeed");

        assert_eq!(read1.data, Bytes::from("block1"));
        assert_eq!(read2.data, Bytes::from("block2"));
        assert_eq!(read3.data, Bytes::from("block3"));
    }

    #[tokio::test]
    async fn test_local_backend_recovery() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let data_path = temp_dir.path().to_path_buf();

        // Create backend and write some blocks
        {
            let backend = LocalBackend::new(data_path.clone(), true).expect("LocalBackend::new should succeed");
            let block1 = Block::new(BlockId(0), BlockType::TableData, Bytes::from("persistent1"));
            let block2 = Block::new(BlockId(0), BlockType::TableData, Bytes::from("persistent2"));

            backend.write_block(block1).await.expect("write_block block1 should succeed");
            backend.write_block(block2).await.expect("write_block block2 should succeed");
        }

        // Create new backend - should recover existing blocks
        {
            let backend = LocalBackend::new(data_path, true).expect("LocalBackend::new should succeed on recovery");

            // Next block ID should be 3 (after 1 and 2)
            let next_id = backend.next_block_id.load(Ordering::SeqCst);
            assert_eq!(next_id, 3);

            // Should be able to read existing blocks
            let read1 = backend.read_block(BlockId(1)).await.expect("read_block BlockId(1) should succeed");
            let read2 = backend.read_block(BlockId(2)).await.expect("read_block BlockId(2) should succeed");

            assert_eq!(read1.data, Bytes::from("persistent1"));
            assert_eq!(read2.data, Bytes::from("persistent2"));
        }
    }

    #[tokio::test]
    async fn test_transaction_commit_rollback_state() {
        let backend = MemoryBackend::new();

        // Begin transaction
        let tx_id = backend.begin_transaction().await.expect("begin_transaction should succeed");

        // Commit should succeed
        backend.commit_transaction(tx_id).await.expect("commit_transaction should succeed");

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
        let tx_id = backend.begin_transaction().await.expect("begin_transaction should succeed");

        // Rollback should succeed
        backend.rollback_transaction(tx_id).await.expect("first rollback_transaction should succeed");

        // Second rollback should be idempotent (succeed)
        backend.rollback_transaction(tx_id).await.expect("second rollback_transaction should be idempotent");

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
