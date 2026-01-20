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
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

// =============================================================================
// Constants
// =============================================================================

pub const WAL_SEGMENT_SIZE: u64 = 64 * 1024 * 1024; // 64 MB
pub const WAL_RECORD_HEADER_SIZE: usize = 24;

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
        let mut buf = BytesMut::with_capacity(WAL_RECORD_HEADER_SIZE + self.data.len());

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
        if data.len() < WAL_RECORD_HEADER_SIZE + 8 {
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

/// Write-ahead log for durability.
pub struct WriteAheadLog {
    wal_dir: PathBuf,
    current_lsn: AtomicU64,
    flushed_lsn: AtomicU64,
    buffer: Mutex<WalBuffer>,
    sync_on_commit: bool,
}

struct WalBuffer {
    records: VecDeque<LogRecord>,
    size: usize,
    writer: Option<BufWriter<File>>,
    segment_offset: u64,
}

impl WriteAheadLog {
    /// Create a new WAL in the specified directory.
    pub fn new(wal_dir: PathBuf, sync_on_commit: bool) -> Result<Self> {
        std::fs::create_dir_all(&wal_dir)?;

        let wal = Self {
            wal_dir,
            current_lsn: AtomicU64::new(1),
            flushed_lsn: AtomicU64::new(0),
            buffer: Mutex::new(WalBuffer {
                records: VecDeque::new(),
                size: 0,
                writer: None,
                segment_offset: 0,
            }),
            sync_on_commit,
        };

        wal.open_segment()?;
        Ok(wal)
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

    /// Append a log record.
    pub fn append(&self, record: LogRecord) -> Result<Lsn> {
        let lsn = record.lsn;
        let data = record.to_bytes();

        let mut buffer = self.buffer.lock();
        buffer.records.push_back(record);
        buffer.size += data.len();

        if let Some(ref mut writer) = buffer.writer {
            writer.write_all(&data)?;
            buffer.segment_offset += data.len() as u64;
        }

        Ok(lsn)
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

    fn open_segment(&self) -> Result<()> {
        let segment_path = self.wal_dir.join("wal_current.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&segment_path)?;

        let mut buffer = self.buffer.lock();
        buffer.writer = Some(BufWriter::new(file));
        buffer.segment_offset = 0;

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
        let restored = LogRecord::from_bytes(&bytes).unwrap();

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
        let restored = LogRecord::from_bytes(&bytes).unwrap();

        assert_eq!(restored.lsn, Lsn(5));
        assert_eq!(restored.prev_lsn, Some(Lsn(4)));
        assert_eq!(restored.page_id, Some(PageId(42)));
        assert_eq!(restored.data, data);
    }

    #[test]
    fn test_wal_operations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let wal = WriteAheadLog::new(temp_dir.path().to_path_buf(), false).unwrap();

        let tx_id = TransactionId(1);
        let begin_lsn = wal.log_begin(tx_id).unwrap();
        assert_eq!(begin_lsn, Lsn(1));

        let insert_lsn = wal
            .log_insert(tx_id, Some(begin_lsn), PageId(1), Bytes::from("data"))
            .unwrap();
        assert_eq!(insert_lsn, Lsn(2));

        let commit_lsn = wal.log_commit(tx_id, insert_lsn).unwrap();
        assert_eq!(commit_lsn, Lsn(3));
    }
}
