//! Aegis Transaction - MVCC Transaction Management
//!
//! Multi-Version Concurrency Control implementation providing snapshot isolation
//! and serializable transactions. Manages transaction lifecycles, version chains,
//! and conflict detection.
//!
//! Key Features:
//! - Snapshot isolation with consistent reads
//! - Optimistic concurrency control
//! - Transaction version tracking
//! - Conflict detection and resolution
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use aegis_common::{TransactionId, Result, AegisError};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// =============================================================================
// Transaction State
// =============================================================================

/// Current state of a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    Active,
    Preparing,
    Committed,
    Aborted,
}

/// Isolation level for transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    #[default]
    RepeatableRead,
    Serializable,
}

// =============================================================================
// Transaction
// =============================================================================

/// A database transaction with MVCC support.
#[derive(Debug)]
pub struct Transaction {
    pub id: TransactionId,
    pub state: TransactionState,
    pub isolation_level: IsolationLevel,
    pub start_timestamp: u64,
    pub commit_timestamp: Option<u64>,
    pub snapshot: Snapshot,
    pub write_set: HashSet<VersionKey>,
    pub read_set: HashSet<VersionKey>,
    pub locks_held: Vec<LockRequest>,
    pub started_at: Instant,
}

impl Transaction {
    pub fn new(
        id: TransactionId,
        isolation_level: IsolationLevel,
        start_timestamp: u64,
        active_transactions: HashSet<TransactionId>,
    ) -> Self {
        Self {
            id,
            state: TransactionState::Active,
            isolation_level,
            start_timestamp,
            commit_timestamp: None,
            snapshot: Snapshot {
                timestamp: start_timestamp,
                active_transactions,
            },
            write_set: HashSet::new(),
            read_set: HashSet::new(),
            locks_held: Vec::new(),
            started_at: Instant::now(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.state == TransactionState::Active
    }

    pub fn duration(&self) -> Duration {
        self.started_at.elapsed()
    }

    pub fn add_to_write_set(&mut self, key: VersionKey) {
        self.write_set.insert(key);
    }

    pub fn add_to_read_set(&mut self, key: VersionKey) {
        self.read_set.insert(key);
    }
}

// =============================================================================
// Snapshot
// =============================================================================

/// A consistent snapshot for transaction reads.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub timestamp: u64,
    pub active_transactions: HashSet<TransactionId>,
}

impl Snapshot {
    /// Check if a version is visible to this snapshot.
    pub fn is_visible(&self, version: &Version) -> bool {
        match version.state {
            VersionState::Committed(commit_ts) => {
                commit_ts <= self.timestamp
                    && !self.active_transactions.contains(&version.created_by)
            }
            VersionState::Active => false,
            VersionState::Aborted => false,
        }
    }
}

// =============================================================================
// Version Management
// =============================================================================

/// Key identifying a specific version of a row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionKey {
    pub table_id: u32,
    pub row_id: u64,
}

/// State of a version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionState {
    Active,
    Committed(u64),
    Aborted,
}

/// A single version in the version chain.
#[derive(Debug, Clone)]
pub struct Version {
    pub key: VersionKey,
    pub created_by: TransactionId,
    pub state: VersionState,
    pub data: Vec<u8>,
    pub prev_version: Option<Box<Version>>,
}

impl Version {
    pub fn new(key: VersionKey, created_by: TransactionId, data: Vec<u8>) -> Self {
        Self {
            key,
            created_by,
            state: VersionState::Active,
            data,
            prev_version: None,
        }
    }

    pub fn commit(&mut self, commit_timestamp: u64) {
        self.state = VersionState::Committed(commit_timestamp);
    }

    pub fn abort(&mut self) {
        self.state = VersionState::Aborted;
    }
}

// =============================================================================
// Lock Management
// =============================================================================

/// Type of lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    Shared,
    Exclusive,
    IntentShared,
    IntentExclusive,
    Update,
}

impl LockMode {
    /// Check if two lock modes are compatible.
    pub fn is_compatible(&self, other: &LockMode) -> bool {
        use LockMode::*;
        matches!(
            (self, other),
            (Shared, Shared)
                | (Shared, IntentShared)
                | (IntentShared, Shared)
                | (IntentShared, IntentShared)
                | (IntentShared, IntentExclusive)
                | (IntentExclusive, IntentShared)
                | (IntentExclusive, IntentExclusive)
        )
    }
}

/// A lock request.
#[derive(Debug, Clone)]
pub struct LockRequest {
    pub tx_id: TransactionId,
    pub key: VersionKey,
    pub mode: LockMode,
    pub granted: bool,
}

/// Entry in the lock table.
#[derive(Debug, Default)]
struct LockEntry {
    holders: Vec<LockRequest>,
    waiters: Vec<LockRequest>,
}

/// Lock manager for concurrency control.
pub struct LockManager {
    locks: RwLock<HashMap<VersionKey, LockEntry>>,
    timeout: Duration,
}

impl LockManager {
    pub fn new(timeout: Duration) -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
            timeout,
        }
    }

    /// Acquire a lock, blocking if necessary.
    pub fn acquire(&self, request: LockRequest) -> Result<()> {
        let start = Instant::now();

        loop {
            {
                let mut locks = self.locks.write();
                let entry = locks.entry(request.key.clone()).or_default();

                let can_grant = entry
                    .holders
                    .iter()
                    .all(|h| h.tx_id == request.tx_id || h.mode.is_compatible(&request.mode));

                if can_grant {
                    entry.holders.push(LockRequest {
                        granted: true,
                        ..request.clone()
                    });
                    return Ok(());
                }

                if !entry.waiters.iter().any(|w| w.tx_id == request.tx_id) {
                    entry.waiters.push(request.clone());
                }
            }

            if start.elapsed() > self.timeout {
                self.release_waiter(&request);
                return Err(AegisError::LockTimeout);
            }

            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Try to acquire a lock without blocking.
    pub fn try_acquire(&self, request: LockRequest) -> Result<bool> {
        let mut locks = self.locks.write();
        let entry = locks.entry(request.key.clone()).or_default();

        let can_grant = entry
            .holders
            .iter()
            .all(|h| h.tx_id == request.tx_id || h.mode.is_compatible(&request.mode));

        if can_grant {
            entry.holders.push(LockRequest {
                granted: true,
                ..request
            });
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Release a lock.
    pub fn release(&self, tx_id: TransactionId, key: &VersionKey) {
        let mut locks = self.locks.write();

        if let Some(entry) = locks.get_mut(key) {
            entry.holders.retain(|h| h.tx_id != tx_id);

            while !entry.waiters.is_empty() {
                let waiter = entry.waiters.remove(0);
                let can_grant = entry
                    .holders
                    .iter()
                    .all(|h| h.mode.is_compatible(&waiter.mode));

                if can_grant {
                    entry.holders.push(LockRequest {
                        granted: true,
                        ..waiter
                    });
                } else {
                    entry.waiters.insert(0, waiter);
                    break;
                }
            }

            if entry.holders.is_empty() && entry.waiters.is_empty() {
                locks.remove(key);
            }
        }
    }

    /// Release all locks held by a transaction.
    pub fn release_all(&self, tx_id: TransactionId) {
        let mut locks = self.locks.write();
        let keys: Vec<_> = locks.keys().cloned().collect();

        for key in keys {
            if let Some(entry) = locks.get_mut(&key) {
                entry.holders.retain(|h| h.tx_id != tx_id);
                entry.waiters.retain(|w| w.tx_id != tx_id);

                if entry.holders.is_empty() && entry.waiters.is_empty() {
                    locks.remove(&key);
                }
            }
        }
    }

    fn release_waiter(&self, request: &LockRequest) {
        let mut locks = self.locks.write();
        if let Some(entry) = locks.get_mut(&request.key) {
            entry.waiters.retain(|w| w.tx_id != request.tx_id);
        }
    }
}

// =============================================================================
// Transaction Manager
// =============================================================================

/// Manages all active transactions.
pub struct TransactionManager {
    transactions: RwLock<HashMap<TransactionId, Transaction>>,
    next_tx_id: AtomicU64,
    next_timestamp: AtomicU64,
    lock_manager: LockManager,
    versions: RwLock<HashMap<VersionKey, Version>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            transactions: RwLock::new(HashMap::new()),
            next_tx_id: AtomicU64::new(1),
            next_timestamp: AtomicU64::new(1),
            lock_manager: LockManager::new(Duration::from_secs(30)),
            versions: RwLock::new(HashMap::new()),
        }
    }

    /// Begin a new transaction.
    pub fn begin(&self, isolation_level: IsolationLevel) -> Result<TransactionId> {
        let tx_id = TransactionId(self.next_tx_id.fetch_add(1, Ordering::SeqCst));
        let start_ts = self.next_timestamp.fetch_add(1, Ordering::SeqCst);

        let active_txs: HashSet<_> = self
            .transactions
            .read()
            .iter()
            .filter(|(_, tx)| tx.is_active())
            .map(|(id, _)| *id)
            .collect();

        let transaction = Transaction::new(tx_id, isolation_level, start_ts, active_txs);

        self.transactions.write().insert(tx_id, transaction);

        Ok(tx_id)
    }

    /// Commit a transaction.
    pub fn commit(&self, tx_id: TransactionId) -> Result<()> {
        let commit_ts = self.next_timestamp.fetch_add(1, Ordering::SeqCst);

        {
            let mut txs = self.transactions.write();
            let tx = txs
                .get_mut(&tx_id)
                .ok_or_else(|| AegisError::Transaction("Transaction not found".to_string()))?;

            if tx.state != TransactionState::Active {
                return Err(AegisError::Transaction("Transaction not active".to_string()));
            }

            if tx.isolation_level == IsolationLevel::Serializable {
                self.validate_serializable(tx)?;
            }

            tx.state = TransactionState::Preparing;
            tx.commit_timestamp = Some(commit_ts);
        }

        {
            let mut versions = self.versions.write();
            let txs = self.transactions.read();
            let tx = txs.get(&tx_id).unwrap();

            for key in &tx.write_set {
                if let Some(version) = versions.get_mut(key) {
                    if version.created_by == tx_id {
                        version.commit(commit_ts);
                    }
                }
            }
        }

        {
            let mut txs = self.transactions.write();
            if let Some(tx) = txs.get_mut(&tx_id) {
                tx.state = TransactionState::Committed;
            }
        }

        self.lock_manager.release_all(tx_id);

        Ok(())
    }

    /// Abort a transaction.
    pub fn abort(&self, tx_id: TransactionId) -> Result<()> {
        {
            let mut txs = self.transactions.write();
            let tx = txs
                .get_mut(&tx_id)
                .ok_or_else(|| AegisError::Transaction("Transaction not found".to_string()))?;

            tx.state = TransactionState::Aborted;
        }

        {
            let mut versions = self.versions.write();
            let txs = self.transactions.read();
            let tx = txs.get(&tx_id).unwrap();

            for key in &tx.write_set {
                if let Some(version) = versions.get_mut(key) {
                    if version.created_by == tx_id {
                        version.abort();
                    }
                }
            }
        }

        self.lock_manager.release_all(tx_id);

        Ok(())
    }

    /// Read a version visible to the transaction.
    pub fn read(&self, tx_id: TransactionId, key: &VersionKey) -> Result<Option<Vec<u8>>> {
        let txs = self.transactions.read();
        let tx = txs
            .get(&tx_id)
            .ok_or_else(|| AegisError::Transaction("Transaction not found".to_string()))?;

        if !tx.is_active() {
            return Err(AegisError::Transaction("Transaction not active".to_string()));
        }

        let versions = self.versions.read();
        if let Some(version) = versions.get(key) {
            if tx.snapshot.is_visible(version) {
                return Ok(Some(version.data.clone()));
            }

            let mut current = version.prev_version.as_ref();
            while let Some(v) = current {
                if tx.snapshot.is_visible(v) {
                    return Ok(Some(v.data.clone()));
                }
                current = v.prev_version.as_ref();
            }
        }

        Ok(None)
    }

    /// Write a new version.
    pub fn write(&self, tx_id: TransactionId, key: VersionKey, data: Vec<u8>) -> Result<()> {
        {
            let mut txs = self.transactions.write();
            let tx = txs
                .get_mut(&tx_id)
                .ok_or_else(|| AegisError::Transaction("Transaction not found".to_string()))?;

            if !tx.is_active() {
                return Err(AegisError::Transaction("Transaction not active".to_string()));
            }

            tx.add_to_write_set(key.clone());
        }

        let lock_request = LockRequest {
            tx_id,
            key: key.clone(),
            mode: LockMode::Exclusive,
            granted: false,
        };
        self.lock_manager.acquire(lock_request)?;

        {
            let txs = self.transactions.read();
            let tx = txs.get(&tx_id).unwrap();
            tx.locks_held.len(); // Just to use tx
        }

        let mut versions = self.versions.write();
        let new_version = Version::new(key.clone(), tx_id, data);

        if let Some(existing) = versions.remove(&key) {
            let mut new_v = new_version;
            new_v.prev_version = Some(Box::new(existing));
            versions.insert(key, new_v);
        } else {
            versions.insert(key, new_version);
        }

        Ok(())
    }

    /// Delete a version.
    pub fn delete(&self, tx_id: TransactionId, key: &VersionKey) -> Result<()> {
        self.write(tx_id, key.clone(), Vec::new())
    }

    /// Get transaction statistics.
    pub fn stats(&self) -> TransactionStats {
        let txs = self.transactions.read();
        let versions = self.versions.read();
        let mut active = 0;
        let mut committed = 0;
        let mut aborted = 0;

        for tx in txs.values() {
            match tx.state {
                TransactionState::Active | TransactionState::Preparing => active += 1,
                TransactionState::Committed => committed += 1,
                TransactionState::Aborted => aborted += 1,
            }
        }

        // Count total versions (including history)
        let mut version_count = 0;
        for version in versions.values() {
            version_count += 1;
            let mut prev = version.prev_version.as_ref();
            while let Some(v) = prev {
                version_count += 1;
                prev = v.prev_version.as_ref();
            }
        }

        TransactionStats {
            active,
            committed,
            aborted,
            total: txs.len(),
            version_count,
        }
    }

    // ==========================================================================
    // Garbage Collection
    // ==========================================================================

    /// Get the minimum active timestamp (low watermark).
    /// Versions older than this are potentially garbage.
    fn get_min_active_timestamp(&self) -> u64 {
        let txs = self.transactions.read();
        txs.values()
            .filter(|tx| tx.is_active())
            .map(|tx| tx.start_timestamp)
            .min()
            .unwrap_or(u64::MAX)
    }

    /// Run garbage collection to remove old versions that are no longer visible.
    /// Returns the number of versions collected.
    pub fn run_gc(&self) -> GcStats {
        let min_ts = self.get_min_active_timestamp();
        let mut versions_collected = 0;

        // Clean up old version chains
        {
            let mut versions = self.versions.write();
            for version in versions.values_mut() {
                versions_collected += Self::gc_version_chain(version, min_ts);
            }
        }

        // Clean up old completed transactions
        let transactions_cleaned = {
            let mut txs = self.transactions.write();
            let to_remove: Vec<TransactionId> = txs
                .iter()
                .filter(|(_, tx)| {
                    match tx.state {
                        TransactionState::Committed | TransactionState::Aborted => {
                            // Remove if older than min active timestamp
                            tx.commit_timestamp.unwrap_or(tx.start_timestamp) < min_ts
                        }
                        _ => false,
                    }
                })
                .map(|(id, _)| *id)
                .collect();

            let count = to_remove.len();
            for id in to_remove {
                txs.remove(&id);
            }
            count
        };

        GcStats {
            versions_collected,
            transactions_cleaned,
            min_active_timestamp: min_ts,
        }
    }

    /// Recursively garbage collect a version chain.
    /// Returns the number of versions removed.
    ///
    /// We can only remove a version if:
    /// 1. The current version is committed (has a valid successor)
    /// 2. The previous version's commit_ts is less than min_ts
    /// 3. The current version's commit_ts is also less than min_ts
    ///    (meaning no active transaction could need to see the previous version)
    fn gc_version_chain(version: &mut Version, min_ts: u64) -> usize {
        let mut collected = 0;

        // Check if we can truncate the version chain
        if let Some(ref mut prev) = version.prev_version {
            // First, recursively GC the older versions
            collected += Self::gc_version_chain(prev, min_ts);

            // Handle aborted versions - they can always be removed
            if prev.state == VersionState::Aborted {
                version.prev_version = prev.prev_version.take();
                collected += 1;
                return collected;
            }

            // For committed versions, we can only remove the previous version if:
            // - The current version is committed
            // - Both versions have commit timestamps less than min_ts
            // This ensures no active transaction needs to see the older version
            if let VersionState::Committed(curr_commit_ts) = version.state {
                if let VersionState::Committed(prev_commit_ts) = prev.state {
                    // Only remove if the current version is also old enough
                    // that any transaction needing to read would see the current version
                    if prev_commit_ts < min_ts && curr_commit_ts < min_ts {
                        version.prev_version = None;
                        collected += 1;
                    }
                }
            }
        }

        collected
    }

    /// Run garbage collection with a threshold.
    /// Only runs if there are more than `threshold` versions.
    pub fn run_gc_if_needed(&self, threshold: usize) -> Option<GcStats> {
        let stats = self.stats();
        if stats.version_count > threshold {
            Some(self.run_gc())
        } else {
            None
        }
    }

    fn validate_serializable(&self, tx: &Transaction) -> Result<()> {
        let txs = self.transactions.read();

        for other_tx in txs.values() {
            if other_tx.id == tx.id {
                continue;
            }

            if other_tx.state != TransactionState::Committed {
                continue;
            }

            if let Some(commit_ts) = other_tx.commit_timestamp {
                if commit_ts > tx.start_timestamp {
                    for read_key in &tx.read_set {
                        if other_tx.write_set.contains(read_key) {
                            return Err(AegisError::SerializationFailure);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Statistics
// =============================================================================

/// Transaction manager statistics.
#[derive(Debug, Clone)]
pub struct TransactionStats {
    pub active: usize,
    pub committed: usize,
    pub aborted: usize,
    pub total: usize,
    /// Total number of versions stored (including historical versions)
    pub version_count: usize,
}

/// Statistics from a garbage collection run.
#[derive(Debug, Clone)]
pub struct GcStats {
    /// Number of old versions removed from version chains
    pub versions_collected: usize,
    /// Number of completed transactions removed
    pub transactions_cleaned: usize,
    /// The minimum active timestamp used as the low watermark
    pub min_active_timestamp: u64,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_lifecycle() {
        let tm = TransactionManager::new();

        let tx_id = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        assert!(tm.transactions.read().get(&tx_id).unwrap().is_active());

        tm.commit(tx_id).unwrap();
        assert_eq!(
            tm.transactions.read().get(&tx_id).unwrap().state,
            TransactionState::Committed
        );
    }

    #[test]
    fn test_transaction_abort() {
        let tm = TransactionManager::new();

        let tx_id = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.abort(tx_id).unwrap();

        assert_eq!(
            tm.transactions.read().get(&tx_id).unwrap().state,
            TransactionState::Aborted
        );
    }

    #[test]
    fn test_mvcc_read_write() {
        let tm = TransactionManager::new();

        let tx1 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        let key = VersionKey {
            table_id: 1,
            row_id: 1,
        };

        tm.write(tx1, key.clone(), b"hello".to_vec()).unwrap();
        tm.commit(tx1).unwrap();

        let tx2 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        let data = tm.read(tx2, &key).unwrap();
        assert_eq!(data, Some(b"hello".to_vec()));
        tm.commit(tx2).unwrap();
    }

    #[test]
    fn test_snapshot_isolation() {
        let tm = TransactionManager::new();

        let key = VersionKey {
            table_id: 1,
            row_id: 1,
        };

        let tx1 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.write(tx1, key.clone(), b"v1".to_vec()).unwrap();
        tm.commit(tx1).unwrap();

        let tx2 = tm.begin(IsolationLevel::RepeatableRead).unwrap();

        let tx3 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.write(tx3, key.clone(), b"v2".to_vec()).unwrap();
        tm.commit(tx3).unwrap();

        let data = tm.read(tx2, &key).unwrap();
        assert_eq!(data, Some(b"v1".to_vec()));

        tm.commit(tx2).unwrap();
    }

    #[test]
    fn test_lock_compatibility() {
        assert!(LockMode::Shared.is_compatible(&LockMode::Shared));
        assert!(!LockMode::Shared.is_compatible(&LockMode::Exclusive));
        assert!(!LockMode::Exclusive.is_compatible(&LockMode::Exclusive));
        assert!(!LockMode::Exclusive.is_compatible(&LockMode::Shared));
    }

    #[test]
    fn test_lock_manager() {
        let lm = LockManager::new(Duration::from_secs(1));
        let key = VersionKey {
            table_id: 1,
            row_id: 1,
        };

        let req1 = LockRequest {
            tx_id: TransactionId(1),
            key: key.clone(),
            mode: LockMode::Shared,
            granted: false,
        };

        assert!(lm.try_acquire(req1).unwrap());

        let req2 = LockRequest {
            tx_id: TransactionId(2),
            key: key.clone(),
            mode: LockMode::Shared,
            granted: false,
        };
        assert!(lm.try_acquire(req2).unwrap());

        let req3 = LockRequest {
            tx_id: TransactionId(3),
            key: key.clone(),
            mode: LockMode::Exclusive,
            granted: false,
        };
        assert!(!lm.try_acquire(req3).unwrap());

        lm.release(TransactionId(1), &key);
        lm.release(TransactionId(2), &key);

        let req4 = LockRequest {
            tx_id: TransactionId(3),
            key: key.clone(),
            mode: LockMode::Exclusive,
            granted: false,
        };
        assert!(lm.try_acquire(req4).unwrap());
    }

    #[test]
    fn test_gc_cleans_old_transactions() {
        let tm = TransactionManager::new();

        // Create and commit several transactions
        let tx1 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.commit(tx1).unwrap();

        let tx2 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.commit(tx2).unwrap();

        let tx3 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.abort(tx3).unwrap();

        // All completed transactions should be present
        assert_eq!(tm.stats().total, 3);
        assert_eq!(tm.stats().committed, 2);
        assert_eq!(tm.stats().aborted, 1);

        // No active transactions, so GC should clean all completed ones
        let gc_stats = tm.run_gc();
        assert_eq!(gc_stats.transactions_cleaned, 3);

        // Transactions should be cleaned up
        assert_eq!(tm.stats().total, 0);
    }

    #[test]
    fn test_gc_preserves_active_transaction_visible_versions() {
        let tm = TransactionManager::new();
        let key = VersionKey { table_id: 1, row_id: 1 };

        // Create first version
        let tx1 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.write(tx1, key.clone(), b"v1".to_vec()).unwrap();
        tm.commit(tx1).unwrap();

        // Start a long-running transaction
        let tx_long = tm.begin(IsolationLevel::RepeatableRead).unwrap();

        // Create second version
        let tx2 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.write(tx2, key.clone(), b"v2".to_vec()).unwrap();
        tm.commit(tx2).unwrap();

        // Long transaction should still see v1
        let data = tm.read(tx_long, &key).unwrap();
        assert_eq!(data, Some(b"v1".to_vec()));

        // GC should not remove v1 because tx_long still needs it
        let _gc_stats = tm.run_gc();
        // tx1 and tx2 are committed and older than tx_long, they can be cleaned
        // but the version chain should be preserved for tx_long

        // Long transaction should still be able to read v1
        let data = tm.read(tx_long, &key).unwrap();
        assert_eq!(data, Some(b"v1".to_vec()));

        tm.commit(tx_long).unwrap();
    }

    #[test]
    fn test_gc_removes_aborted_versions() {
        let tm = TransactionManager::new();
        let key = VersionKey { table_id: 1, row_id: 1 };

        // Create first version
        let tx1 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.write(tx1, key.clone(), b"v1".to_vec()).unwrap();
        tm.commit(tx1).unwrap();

        // Create and abort a version
        let tx2 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.write(tx2, key.clone(), b"v2_aborted".to_vec()).unwrap();
        tm.abort(tx2).unwrap();

        // Create third version
        let tx3 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.write(tx3, key.clone(), b"v3".to_vec()).unwrap();
        tm.commit(tx3).unwrap();

        // GC should clean up aborted versions
        let gc_stats = tm.run_gc();
        assert!(gc_stats.versions_collected > 0 || gc_stats.transactions_cleaned > 0);

        // New transaction should see v3
        let tx4 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        let data = tm.read(tx4, &key).unwrap();
        assert_eq!(data, Some(b"v3".to_vec()));
        tm.commit(tx4).unwrap();
    }

    #[test]
    fn test_stats_includes_version_count() {
        let tm = TransactionManager::new();
        let key = VersionKey { table_id: 1, row_id: 1 };

        assert_eq!(tm.stats().version_count, 0);

        let tx1 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.write(tx1, key.clone(), b"v1".to_vec()).unwrap();
        tm.commit(tx1).unwrap();

        assert_eq!(tm.stats().version_count, 1);

        let tx2 = tm.begin(IsolationLevel::RepeatableRead).unwrap();
        tm.write(tx2, key.clone(), b"v2".to_vec()).unwrap();
        tm.commit(tx2).unwrap();

        // Now we have 2 versions in the chain
        assert_eq!(tm.stats().version_count, 2);
    }

    #[test]
    fn test_run_gc_if_needed() {
        let tm = TransactionManager::new();
        let key = VersionKey { table_id: 1, row_id: 1 };

        // Should not run GC with high threshold
        assert!(tm.run_gc_if_needed(100).is_none());

        // Create some versions
        for i in 0..5 {
            let tx = tm.begin(IsolationLevel::RepeatableRead).unwrap();
            tm.write(tx, key.clone(), format!("v{}", i).into_bytes()).unwrap();
            tm.commit(tx).unwrap();
        }

        // Should now have 5 versions in the chain
        assert_eq!(tm.stats().version_count, 5);

        // Should run GC with low threshold
        let result = tm.run_gc_if_needed(3);
        assert!(result.is_some());
    }
}
