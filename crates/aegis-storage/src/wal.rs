//! Aegis WAL - Write-Ahead Logging
//!
//! Write-ahead log for durability and crash recovery. All modifications are
//! logged before being applied to data pages, ensuring ACID durability
//! guarantees even in the face of system failures.
//!
//! Key Features:
//! - Sequential write optimization for high throughput
//! - Log sequence numbers (LSN) for ordering and recovery
//! - Checkpoint support for recovery time optimization
//! - Segment-based log file management
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use aegis_common::{Lsn, PageId, TransactionId, Result, AegisError};
use bytes::{Bytes, BytesMut, BufMut, Buf};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

// =============================================================================
// Constants
// =============================================================================

pub const WAL_SEGMENT_SIZE: u64 = 64 * 1024 * 1024; // 64 MB
/// Size of the fixed record header before variable data:
/// lsn(8) + prev_lsn(8) + tx_id(8) + type(1) + has_page(1) + padding(2) + page_id(8) + data_len(4) = 40
pub const WAL_RECORD_HEADER_SIZE: usize = 40;

// =============================================================================
// Log Record Types
// =============================================================================

/// Type of WAL record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum LogRecordType {
    Begin = 1,
    Commit = 2,
    Abort = 3,
    Insert = 4,
    Update = 5,
    Delete = 6,
    Checkpoint = 7,
    CompensationLogRecord = 8,
}

impl From<u8> for LogRecordType {
    fn from(value: u8) -> Self {
        match value {
            1 => LogRecordType::Begin,
            2 => LogRecordType::Commit,
            3 => LogRecordType::Abort,
            4 => LogRecordType::Insert,
            5 => LogRecordType::Update,
            6 => LogRecordType::Delete,
            7 => LogRecordType::Checkpoint,
            8 => LogRecordType::CompensationLogRecord,
            _ => LogRecordType::Begin,
        }
    }
}

// =============================================================================
// Log Record
// =============================================================================

/// A single record in the write-ahead log.
#[derive(Debug, Clone)]
pub struct LogRecord {
    pub lsn: Lsn,
    pub prev_lsn: Option<Lsn>,
    pub tx_id: TransactionId,
    pub record_type: LogRecordType,
    pub page_id: Option<PageId>,
    pub data: Bytes,
}

impl LogRecord {
    /// Create a new transaction begin record.
    pub fn begin(lsn: Lsn, tx_id: TransactionId) -> Self {
        Self {
            lsn,
            prev_lsn: None,
            tx_id,
            record_type: LogRecordType::Begin,
            page_id: None,
            data: Bytes::new(),
        }
    }

    /// Create a new transaction commit record.
    pub fn commit(lsn: Lsn, prev_lsn: Lsn, tx_id: TransactionId) -> Self {
        Self {
            lsn,
            prev_lsn: Some(prev_lsn),
            tx_id,
            record_type: LogRecordType::Commit,
            page_id: None,
            data: Bytes::new(),
        }
    }

    /// Create a new transaction abort record.
    pub fn abort(lsn: Lsn, prev_lsn: Lsn, tx_id: TransactionId) -> Self {
        Self {
            lsn,
            prev_lsn: Some(prev_lsn),
            tx_id,
            record_type: LogRecordType::Abort,
            page_id: None,
            data: Bytes::new(),
        }
    }

    /// Create a data modification record.
    pub fn data_record(
        lsn: Lsn,
        prev_lsn: Option<Lsn>,
        tx_id: TransactionId,
        record_type: LogRecordType,
        page_id: PageId,
        data: Bytes,
    ) -> Self {
        Self {
            lsn,
            prev_lsn,
            tx_id,
            record_type,
            page_id: Some(page_id),
            data,
        }
    }

    /// Serialize the record to bytes.
    pub fn to_bytes(&self) -> Bytes {
        // header(40) + data + checksum(4)
        let mut buf = BytesMut::with_capacity(WAL_RECORD_HEADER_SIZE + self.data.len() + 4);

        buf.put_u64_le(self.lsn.0);
        buf.put_u64_le(self.prev_lsn.map_or(0, |l| l.0));
        buf.put_u64_le(self.tx_id.0);
        buf.put_u8(self.record_type as u8);
        buf.put_u8(self.page_id.is_some() as u8);
        buf.put_u16_le(0); // padding
        buf.put_u64_le(self.page_id.map_or(0, |p| p.0));
        buf.put_u32_le(self.data.len() as u32);
        buf.put(self.data.clone());

        let checksum = crc32fast::hash(&buf);
        buf.put_u32_le(checksum);

        buf.freeze()
    }

    /// Deserialize a record from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        // Minimum size: header(40) + checksum(4)
        if data.len() < WAL_RECORD_HEADER_SIZE + 4 {
            return Err(AegisError::Corruption("Log record too small".to_string()));
        }

        let mut buf = data;
        let lsn = Lsn(buf.get_u64_le());
        let prev_lsn_raw = buf.get_u64_le();
        let prev_lsn = if prev_lsn_raw == 0 {
            None
        } else {
            Some(Lsn(prev_lsn_raw))
        };
        let tx_id = TransactionId(buf.get_u64_le());
        let record_type = LogRecordType::from(buf.get_u8());
        let has_page_id = buf.get_u8() != 0;
        let _padding = buf.get_u16_le();
        let page_id_raw = buf.get_u64_le();
        let page_id = if has_page_id {
            Some(PageId(page_id_raw))
        } else {
            None
        };
        let data_len = buf.get_u32_le() as usize;

        if buf.remaining() < data_len + 4 {
            return Err(AegisError::Corruption("Log record data truncated".to_string()));
        }

        let record_data = Bytes::copy_from_slice(&buf[..data_len]);
        buf.advance(data_len);

        let stored_checksum = buf.get_u32_le();
        let computed_checksum = crc32fast::hash(&data[..data.len() - 4]);

        if stored_checksum != computed_checksum {
            return Err(AegisError::Corruption("Log record checksum mismatch".to_string()));
        }

        Ok(Self {
            lsn,
            prev_lsn,
            tx_id,
            record_type,
            page_id,
            data: record_data,
        })
    }
}

// =============================================================================
// Write-Ahead Log
// =============================================================================

/// Write-ahead log for durability with segment rotation and crash recovery.
pub struct WriteAheadLog {
    wal_dir: PathBuf,
    current_lsn: AtomicU64,
    flushed_lsn: AtomicU64,
    /// LSN of the last checkpoint
    checkpoint_lsn: AtomicU64,
    buffer: Mutex<WalBuffer>,
    sync_on_commit: bool,
}

struct WalBuffer {
    records: VecDeque<LogRecord>,
    size: usize,
    writer: Option<BufWriter<File>>,
    segment_offset: u64,
    /// Current segment number
    current_segment: u64,
}

/// Result of WAL recovery.
#[derive(Debug)]
pub struct RecoveryResult {
    /// Records that need to be redone (committed transactions)
    pub redo_records: Vec<LogRecord>,
    /// Transaction IDs that were in-progress (need rollback)
    pub incomplete_transactions: HashSet<TransactionId>,
    /// The highest LSN found during recovery
    pub max_lsn: Lsn,
    /// Number of records processed
    pub records_processed: usize,
    /// Number of segments scanned
    pub segments_scanned: usize,
}

impl Default for RecoveryResult {
    fn default() -> Self {
        Self {
            redo_records: Vec::new(),
            incomplete_transactions: HashSet::new(),
            max_lsn: Lsn(0),
            records_processed: 0,
            segments_scanned: 0,
        }
    }
}

/// Checkpoint data stored in checkpoint records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointData {
    /// Active transaction IDs at checkpoint time
    pub active_transactions: Vec<TransactionId>,
    /// Dirty page IDs that need to be flushed
    pub dirty_pages: Vec<PageId>,
}

impl WriteAheadLog {
    /// Create a new WAL in the specified directory.
    pub fn new(wal_dir: PathBuf, sync_on_commit: bool) -> Result<Self> {
        std::fs::create_dir_all(&wal_dir)?;

        // Find the highest segment number
        let current_segment = Self::find_latest_segment(&wal_dir)?;

        let wal = Self {
            wal_dir,
            current_lsn: AtomicU64::new(1),
            flushed_lsn: AtomicU64::new(0),
            checkpoint_lsn: AtomicU64::new(0),
            buffer: Mutex::new(WalBuffer {
                records: VecDeque::new(),
                size: 0,
                writer: None,
                segment_offset: 0,
                current_segment,
            }),
            sync_on_commit,
        };

        wal.open_segment(current_segment)?;
        Ok(wal)
    }

    /// Create a WAL and perform recovery from existing log files.
    pub fn open_and_recover(wal_dir: PathBuf, sync_on_commit: bool) -> Result<(Self, RecoveryResult)> {
        std::fs::create_dir_all(&wal_dir)?;

        // Perform recovery first
        let recovery = Self::recover_from_directory(&wal_dir)?;

        // Create WAL starting from recovered LSN
        let current_segment = Self::find_latest_segment(&wal_dir)?;
        let next_lsn = recovery.max_lsn.0.saturating_add(1).max(1);

        let wal = Self {
            wal_dir,
            current_lsn: AtomicU64::new(next_lsn),
            flushed_lsn: AtomicU64::new(recovery.max_lsn.0),
            checkpoint_lsn: AtomicU64::new(0),
            buffer: Mutex::new(WalBuffer {
                records: VecDeque::new(),
                size: 0,
                writer: None,
                segment_offset: 0,
                current_segment,
            }),
            sync_on_commit,
        };

        wal.open_segment(current_segment)?;
        Ok((wal, recovery))
    }

    /// Find the latest segment number in the WAL directory.
    fn find_latest_segment(wal_dir: &PathBuf) -> Result<u64> {
        let mut max_segment = 0u64;

        if let Ok(entries) = std::fs::read_dir(wal_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some(num_str) = name.strip_prefix("wal_").and_then(|s| s.strip_suffix(".log")) {
                        if let Ok(num) = num_str.parse::<u64>() {
                            max_segment = max_segment.max(num);
                        }
                    }
                }
            }
        }

        Ok(max_segment)
    }

    /// Allocate the next LSN.
    pub fn next_lsn(&self) -> Lsn {
        Lsn(self.current_lsn.fetch_add(1, Ordering::SeqCst))
    }

    /// Get the current LSN.
    pub fn current_lsn(&self) -> Lsn {
        Lsn(self.current_lsn.load(Ordering::SeqCst))
    }

    /// Get the flushed LSN.
    pub fn flushed_lsn(&self) -> Lsn {
        Lsn(self.flushed_lsn.load(Ordering::SeqCst))
    }

    /// Get the checkpoint LSN.
    pub fn checkpoint_lsn(&self) -> Lsn {
        Lsn(self.checkpoint_lsn.load(Ordering::SeqCst))
    }

    /// Append a log record, rotating segment if needed.
    pub fn append(&self, record: LogRecord) -> Result<Lsn> {
        let lsn = record.lsn;
        let data = record.to_bytes();
        let data_len = data.len() as u64;

        let mut buffer = self.buffer.lock();

        // Check if we need to rotate to a new segment
        if buffer.segment_offset + data_len > WAL_SEGMENT_SIZE {
            drop(buffer);
            self.rotate_segment()?;
            buffer = self.buffer.lock();
        }

        buffer.records.push_back(record);
        buffer.size += data.len();

        if let Some(ref mut writer) = buffer.writer {
            writer.write_all(&data)?;
            buffer.segment_offset += data_len;
        }

        Ok(lsn)
    }

    /// Rotate to a new WAL segment.
    fn rotate_segment(&self) -> Result<()> {
        let mut buffer = self.buffer.lock();

        // Flush current segment
        if let Some(ref mut writer) = buffer.writer {
            writer.flush()?;
            if self.sync_on_commit {
                writer.get_ref().sync_all()?;
            }
        }

        // Rename current segment to numbered segment
        let old_path = self.wal_dir.join("wal_current.log");
        let new_segment = buffer.current_segment + 1;
        let new_path = self.wal_dir.join(format!("wal_{:016}.log", buffer.current_segment));

        if old_path.exists() {
            std::fs::rename(&old_path, &new_path)?;
        }

        // Open new segment
        buffer.current_segment = new_segment;
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&old_path)?;

        buffer.writer = Some(BufWriter::new(file));
        buffer.segment_offset = 0;

        tracing::info!("Rotated WAL to segment {}", new_segment);
        Ok(())
    }

    /// Flush all buffered records to disk.
    pub fn flush(&self) -> Result<Lsn> {
        let mut buffer = self.buffer.lock();

        if let Some(ref mut writer) = buffer.writer {
            writer.flush()?;

            if self.sync_on_commit {
                writer.get_ref().sync_all()?;
            }
        }

        let flushed = self.current_lsn.load(Ordering::SeqCst) - 1;
        self.flushed_lsn.store(flushed, Ordering::SeqCst);
        buffer.records.clear();
        buffer.size = 0;

        Ok(Lsn(flushed))
    }

    /// Log a transaction begin.
    pub fn log_begin(&self, tx_id: TransactionId) -> Result<Lsn> {
        let lsn = self.next_lsn();
        let record = LogRecord::begin(lsn, tx_id);
        self.append(record)
    }

    /// Log a transaction commit.
    pub fn log_commit(&self, tx_id: TransactionId, prev_lsn: Lsn) -> Result<Lsn> {
        let lsn = self.next_lsn();
        let record = LogRecord::commit(lsn, prev_lsn, tx_id);
        self.append(record)?;

        if self.sync_on_commit {
            self.flush()?;
        }

        Ok(lsn)
    }

    /// Log a transaction abort.
    pub fn log_abort(&self, tx_id: TransactionId, prev_lsn: Lsn) -> Result<Lsn> {
        let lsn = self.next_lsn();
        let record = LogRecord::abort(lsn, prev_lsn, tx_id);
        self.append(record)
    }

    /// Log an insert operation.
    pub fn log_insert(
        &self,
        tx_id: TransactionId,
        prev_lsn: Option<Lsn>,
        page_id: PageId,
        data: Bytes,
    ) -> Result<Lsn> {
        let lsn = self.next_lsn();
        let record = LogRecord::data_record(lsn, prev_lsn, tx_id, LogRecordType::Insert, page_id, data);
        self.append(record)
    }

    /// Log an update operation.
    pub fn log_update(
        &self,
        tx_id: TransactionId,
        prev_lsn: Option<Lsn>,
        page_id: PageId,
        data: Bytes,
    ) -> Result<Lsn> {
        let lsn = self.next_lsn();
        let record = LogRecord::data_record(lsn, prev_lsn, tx_id, LogRecordType::Update, page_id, data);
        self.append(record)
    }

    /// Log a delete operation.
    pub fn log_delete(
        &self,
        tx_id: TransactionId,
        prev_lsn: Option<Lsn>,
        page_id: PageId,
        data: Bytes,
    ) -> Result<Lsn> {
        let lsn = self.next_lsn();
        let record = LogRecord::data_record(lsn, prev_lsn, tx_id, LogRecordType::Delete, page_id, data);
        self.append(record)
    }

    /// Log a checkpoint with active transaction and dirty page information.
    pub fn log_checkpoint(
        &self,
        active_transactions: Vec<TransactionId>,
        dirty_pages: Vec<PageId>,
    ) -> Result<Lsn> {
        let lsn = self.next_lsn();
        let checkpoint_data = CheckpointData {
            active_transactions,
            dirty_pages,
        };
        let data = serde_json::to_vec(&checkpoint_data)
            .map_err(|e| AegisError::Internal(format!("Failed to serialize checkpoint: {}", e)))?;

        let record = LogRecord {
            lsn,
            prev_lsn: None,
            tx_id: TransactionId(0),
            record_type: LogRecordType::Checkpoint,
            page_id: None,
            data: Bytes::from(data),
        };

        self.append(record)?;
        self.flush()?;

        // Update checkpoint LSN
        self.checkpoint_lsn.store(lsn.0, Ordering::SeqCst);

        tracing::info!("Checkpoint created at LSN {}", lsn.0);
        Ok(lsn)
    }

    /// Truncate WAL segments older than the checkpoint LSN.
    pub fn truncate_before_checkpoint(&self) -> Result<usize> {
        let checkpoint = self.checkpoint_lsn.load(Ordering::SeqCst);
        if checkpoint == 0 {
            return Ok(0);
        }

        let mut removed = 0;
        let buffer = self.buffer.lock();
        let current_segment = buffer.current_segment;
        drop(buffer);

        // Find and remove old segments
        if let Ok(entries) = std::fs::read_dir(&self.wal_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Some(num_str) = name.strip_prefix("wal_").and_then(|s| s.strip_suffix(".log")) {
                        if let Ok(num) = num_str.parse::<u64>() {
                            // Keep at least the current segment and one before
                            if num + 2 < current_segment
                                && std::fs::remove_file(&path).is_ok() {
                                    removed += 1;
                                    tracing::debug!("Removed old WAL segment: {}", name);
                                }
                        }
                    }
                }
            }
        }

        Ok(removed)
    }

    /// Recover from WAL directory, returning records needed for redo/undo.
    fn recover_from_directory(wal_dir: &PathBuf) -> Result<RecoveryResult> {
        let mut result = RecoveryResult::default();
        let mut tx_status: HashMap<TransactionId, bool> = HashMap::new(); // true = committed
        let mut tx_records: HashMap<TransactionId, Vec<LogRecord>> = HashMap::new();

        // Collect all segment files
        let mut segments: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(wal_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("wal_") && name.ends_with(".log") {
                        segments.push(path);
                    }
                }
            }
        }

        // Sort segments by number
        segments.sort();

        // Process each segment
        for segment_path in &segments {
            result.segments_scanned += 1;
            let records = Self::read_segment(segment_path)?;

            for record in records {
                result.records_processed += 1;
                result.max_lsn = result.max_lsn.max(record.lsn);

                match record.record_type {
                    LogRecordType::Begin => {
                        tx_status.insert(record.tx_id, false);
                        tx_records.insert(record.tx_id, Vec::new());
                    }
                    LogRecordType::Commit => {
                        tx_status.insert(record.tx_id, true);
                    }
                    LogRecordType::Abort => {
                        // Mark as complete but not committed - remove from tracking
                        tx_status.remove(&record.tx_id);
                        tx_records.remove(&record.tx_id);
                    }
                    LogRecordType::Insert | LogRecordType::Update | LogRecordType::Delete => {
                        if let Some(records) = tx_records.get_mut(&record.tx_id) {
                            records.push(record.clone());
                        }
                    }
                    LogRecordType::Checkpoint => {
                        // Parse checkpoint data to find active transactions
                        if let Ok(checkpoint) = serde_json::from_slice::<CheckpointData>(&record.data) {
                            for tx_id in checkpoint.active_transactions {
                                tx_status.entry(tx_id).or_insert(false);
                            }
                        }
                    }
                    LogRecordType::CompensationLogRecord => {
                        // CLRs are used during undo - skip for now
                    }
                }
            }
        }

        // Collect redo records for committed transactions
        for (tx_id, committed) in &tx_status {
            if *committed {
                if let Some(records) = tx_records.remove(tx_id) {
                    result.redo_records.extend(records);
                }
            } else {
                result.incomplete_transactions.insert(*tx_id);
            }
        }

        // Sort redo records by LSN
        result.redo_records.sort_by_key(|r| r.lsn);

        tracing::info!(
            "WAL recovery: {} records processed, {} redo records, {} incomplete transactions",
            result.records_processed,
            result.redo_records.len(),
            result.incomplete_transactions.len()
        );

        Ok(result)
    }

    /// Read all records from a segment file.
    fn read_segment(path: &PathBuf) -> Result<Vec<LogRecord>> {
        let mut file = BufReader::new(File::open(path)?);
        let mut records = Vec::new();
        let mut buffer = Vec::new();

        // Read the entire file
        file.read_to_end(&mut buffer)?;

        let mut offset = 0;
        while offset < buffer.len() {
            // Need at least header + checksum
            if buffer.len() - offset < WAL_RECORD_HEADER_SIZE + 4 {
                break;
            }

            // Read data length from header (at offset 36: after lsn, prev_lsn, tx_id, type, has_page, padding, page_id)
            let data_len_offset = offset + 36;
            if data_len_offset + 4 > buffer.len() {
                break;
            }

            let data_len = u32::from_le_bytes([
                buffer[data_len_offset],
                buffer[data_len_offset + 1],
                buffer[data_len_offset + 2],
                buffer[data_len_offset + 3],
            ]) as usize;

            let total_record_len = WAL_RECORD_HEADER_SIZE + data_len + 4; // header + data + checksum

            if offset + total_record_len > buffer.len() {
                break;
            }

            match LogRecord::from_bytes(&buffer[offset..offset + total_record_len]) {
                Ok(record) => {
                    records.push(record);
                    offset += total_record_len;
                }
                Err(e) => {
                    tracing::warn!("Failed to parse WAL record at offset {}: {}", offset, e);
                    break;
                }
            }
        }

        Ok(records)
    }

    fn open_segment(&self, segment_num: u64) -> Result<()> {
        let segment_path = self.wal_dir.join("wal_current.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&segment_path)?;

        // Get current file size
        let metadata = file.metadata()?;
        let offset = metadata.len();

        let mut buffer = self.buffer.lock();
        buffer.writer = Some(BufWriter::new(file));
        buffer.segment_offset = offset;
        buffer.current_segment = segment_num;

        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_record_roundtrip() {
        let record = LogRecord::begin(Lsn(1), TransactionId(100));
        let bytes = record.to_bytes();
        let restored = LogRecord::from_bytes(&bytes).expect("failed to deserialize log record");

        assert_eq!(restored.lsn, Lsn(1));
        assert_eq!(restored.tx_id, TransactionId(100));
        assert_eq!(restored.record_type, LogRecordType::Begin);
    }

    #[test]
    fn test_log_record_with_data() {
        let data = Bytes::from("test data");
        let record = LogRecord::data_record(
            Lsn(5),
            Some(Lsn(4)),
            TransactionId(100),
            LogRecordType::Insert,
            PageId(42),
            data.clone(),
        );

        let bytes = record.to_bytes();
        let restored = LogRecord::from_bytes(&bytes).expect("failed to deserialize log record with data");

        assert_eq!(restored.lsn, Lsn(5));
        assert_eq!(restored.prev_lsn, Some(Lsn(4)));
        assert_eq!(restored.page_id, Some(PageId(42)));
        assert_eq!(restored.data, data);
    }

    #[test]
    fn test_wal_operations() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp directory");
        let wal = WriteAheadLog::new(temp_dir.path().to_path_buf(), false).expect("failed to create WAL");

        let tx_id = TransactionId(1);
        let begin_lsn = wal.log_begin(tx_id).expect("failed to log begin");
        assert_eq!(begin_lsn, Lsn(1));

        let insert_lsn = wal
            .log_insert(tx_id, Some(begin_lsn), PageId(1), Bytes::from("data"))
            .expect("failed to log insert");
        assert_eq!(insert_lsn, Lsn(2));

        let commit_lsn = wal.log_commit(tx_id, insert_lsn).expect("failed to log commit");
        assert_eq!(commit_lsn, Lsn(3));
    }

    #[test]
    fn test_wal_recovery_committed_transaction() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp directory");
        let wal_dir = temp_dir.path().to_path_buf();

        // Create WAL and write a committed transaction
        {
            let wal = WriteAheadLog::new(wal_dir.clone(), true).expect("failed to create WAL");
            let tx_id = TransactionId(1);

            let begin_lsn = wal.log_begin(tx_id).expect("failed to log begin");
            let insert_lsn = wal
                .log_insert(tx_id, Some(begin_lsn), PageId(1), Bytes::from("test data"))
                .expect("failed to log insert");
            wal.log_commit(tx_id, insert_lsn).expect("failed to log commit");
        }

        // Recover and verify
        let (wal, recovery) = WriteAheadLog::open_and_recover(wal_dir, true).expect("failed to recover WAL");
        assert_eq!(recovery.records_processed, 3);
        assert_eq!(recovery.redo_records.len(), 1); // One insert record
        assert!(recovery.incomplete_transactions.is_empty());
        assert_eq!(recovery.max_lsn, Lsn(3));

        // Verify we can continue writing
        let next_lsn = wal.next_lsn();
        assert_eq!(next_lsn, Lsn(4));
    }

    #[test]
    fn test_wal_recovery_incomplete_transaction() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp directory");
        let wal_dir = temp_dir.path().to_path_buf();

        // Create WAL with an incomplete transaction
        {
            let wal = WriteAheadLog::new(wal_dir.clone(), true).expect("failed to create WAL");
            let tx_id = TransactionId(1);

            wal.log_begin(tx_id).expect("failed to log begin");
            wal.log_insert(tx_id, None, PageId(1), Bytes::from("uncommitted")).expect("failed to log insert");
            wal.flush().expect("failed to flush WAL");
            // No commit - transaction is incomplete
        }

        // Recover and verify
        let (_wal, recovery) = WriteAheadLog::open_and_recover(wal_dir, true).expect("failed to recover WAL");
        assert_eq!(recovery.records_processed, 2);
        assert!(recovery.redo_records.is_empty()); // No redo for uncommitted
        assert!(recovery.incomplete_transactions.contains(&TransactionId(1)));
    }

    #[test]
    fn test_wal_checkpoint() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp directory");
        let wal = WriteAheadLog::new(temp_dir.path().to_path_buf(), true).expect("failed to create WAL");

        // Write some transactions
        let tx1 = TransactionId(1);
        let begin1 = wal.log_begin(tx1).expect("failed to log begin");
        wal.log_insert(tx1, Some(begin1), PageId(1), Bytes::from("data1")).expect("failed to log insert");

        // Create checkpoint
        let checkpoint_lsn = wal
            .log_checkpoint(vec![tx1], vec![PageId(1)])
            .expect("failed to log checkpoint");

        assert!(checkpoint_lsn.0 > 0);
        assert_eq!(wal.checkpoint_lsn(), checkpoint_lsn);
    }

    #[test]
    fn test_wal_recovery_with_checkpoint() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp directory");
        let wal_dir = temp_dir.path().to_path_buf();

        // Create WAL with checkpoint
        {
            let wal = WriteAheadLog::new(wal_dir.clone(), true).expect("failed to create WAL");

            // Transaction 1 - committed before checkpoint
            let tx1 = TransactionId(1);
            let begin1 = wal.log_begin(tx1).expect("failed to log begin for tx1");
            let insert1 = wal.log_insert(tx1, Some(begin1), PageId(1), Bytes::from("data1")).expect("failed to log insert for tx1");
            wal.log_commit(tx1, insert1).expect("failed to log commit for tx1");

            // Checkpoint
            wal.log_checkpoint(vec![], vec![]).expect("failed to log checkpoint");

            // Transaction 2 - committed after checkpoint
            let tx2 = TransactionId(2);
            let begin2 = wal.log_begin(tx2).expect("failed to log begin for tx2");
            let insert2 = wal.log_insert(tx2, Some(begin2), PageId(2), Bytes::from("data2")).expect("failed to log insert for tx2");
            wal.log_commit(tx2, insert2).expect("failed to log commit for tx2");
        }

        // Recover
        let (_wal, recovery) = WriteAheadLog::open_and_recover(wal_dir, true).expect("failed to recover WAL");

        // Both transactions should have redo records
        assert_eq!(recovery.redo_records.len(), 2);
        assert!(recovery.incomplete_transactions.is_empty());
    }

    #[test]
    fn test_wal_multiple_transactions() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp directory");
        let wal_dir = temp_dir.path().to_path_buf();

        {
            let wal = WriteAheadLog::new(wal_dir.clone(), true).expect("failed to create WAL");

            // Transaction 1 - committed
            let tx1 = TransactionId(1);
            let begin1 = wal.log_begin(tx1).expect("failed to log begin for tx1");
            let insert1 = wal.log_insert(tx1, Some(begin1), PageId(1), Bytes::from("tx1")).expect("failed to log insert for tx1");
            wal.log_commit(tx1, insert1).expect("failed to log commit for tx1");

            // Transaction 2 - aborted
            let tx2 = TransactionId(2);
            let begin2 = wal.log_begin(tx2).expect("failed to log begin for tx2");
            let insert2 = wal.log_insert(tx2, Some(begin2), PageId(2), Bytes::from("tx2")).expect("failed to log insert for tx2");
            wal.log_abort(tx2, insert2).expect("failed to log abort for tx2");

            // Transaction 3 - committed
            let tx3 = TransactionId(3);
            let begin3 = wal.log_begin(tx3).expect("failed to log begin for tx3");
            let insert3 = wal.log_insert(tx3, Some(begin3), PageId(3), Bytes::from("tx3")).expect("failed to log insert for tx3");
            wal.log_commit(tx3, insert3).expect("failed to log commit for tx3");

            wal.flush().expect("failed to flush WAL");
        }

        let (_wal, recovery) = WriteAheadLog::open_and_recover(wal_dir, true).expect("failed to recover WAL");

        // Only tx1 and tx3 should be in redo (tx2 was aborted)
        assert_eq!(recovery.redo_records.len(), 2);
        assert!(recovery.incomplete_transactions.is_empty());

        // Verify the redo records are from correct transactions
        let tx_ids: std::collections::HashSet<_> = recovery.redo_records
            .iter()
            .map(|r| r.tx_id)
            .collect();
        assert!(tx_ids.contains(&TransactionId(1)));
        assert!(tx_ids.contains(&TransactionId(3)));
        assert!(!tx_ids.contains(&TransactionId(2)));
    }
}
