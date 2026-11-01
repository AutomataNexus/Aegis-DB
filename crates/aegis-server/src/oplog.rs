//! Aegis Redo Log (write-ahead log for KV, documents and timeseries)
//!
//! Every KV, document and timeseries mutation is appended here before the request
//! returns, so a crash or `SIGKILL` between checkpoints loses nothing: on start the
//! server loads the last checkpoint and replays the log on top of it.
//!
//! On disk: `<data_dir>/oplog/<generation>.log`, each record framed as
//! `len (u32 LE) | crc32 (u32 LE) | bincode payload`. A record is written with one
//! `write` call straight to the file (no user-space buffer), so it is in the page
//! cache — and survives the process being killed — as soon as `append` returns; an
//! `fsync` runs every `sync_interval` (or on every append when it is zero) to cover
//! power loss. Replay stops at the first torn or corrupt frame of a segment.
//!
//! Checkpoints: [`OpLog::cut`] takes the apply gate exclusively, rotates to a new
//! generation and runs the caller's capture, so the captured state holds exactly the
//! records of the generations before the cut. Once that state is safely on disk,
//! [`OpLog::retire`] deletes those generations.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// Largest record accepted on replay; anything bigger is treated as a torn frame.
const MAX_RECORD_BYTES: u32 = 256 * 1024 * 1024;

/// One logged mutation. JSON values travel as strings (bincode cannot carry
/// self-describing values); timestamps as Unix nanoseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Op {
    /// The KV entry as it is after the write.
    KvPut {
        key: String,
        value_json: String,
        ttl: Option<u64>,
        created_ns: i64,
        updated_ns: i64,
    },
    KvDelete {
        key: String,
    },
    /// The document as it is after the write (insert or update).
    DocPut {
        collection: String,
        doc_json: String,
    },
    DocDelete {
        collection: String,
        id: String,
    },
    CollectionCreate {
        name: String,
    },
    CollectionDrop {
        name: String,
    },
    /// Points appended to one series: `(timestamp_ns, value)`.
    TsWrite {
        metric: String,
        tags: HashMap<String, String>,
        points: Vec<(i64, f64)>,
    },
}

struct Writer {
    file: File,
    generation: u64,
}

/// What [`OpLog::open`] found on disk.
pub struct Recovered {
    /// Records of every surviving generation, oldest first.
    pub ops: Vec<Op>,
    /// Segments read.
    pub segments: usize,
    /// Segments whose tail held a torn or corrupt frame (the rest of that segment
    /// was skipped).
    pub torn_segments: usize,
}

pub struct OpLog {
    dir: PathBuf,
    writer: Mutex<Writer>,
    /// Shared while a mutation is applied and logged; exclusive for a checkpoint cut.
    gate: RwLock<()>,
    dirty: AtomicBool,
    sync_every_append: bool,
    appended: AtomicU64,
}

impl OpLog {
    /// Open the log in `dir`, returning it with the records to replay. New records
    /// always go to a fresh generation, never onto the tail of an old (possibly
    /// torn) segment.
    pub fn open(dir: &Path, sync_interval: Duration) -> std::io::Result<(Self, Recovered)> {
        std::fs::create_dir_all(dir)?;
        let generations = list_generations(dir)?;
        let mut ops = Vec::new();
        let mut torn_segments = 0;
        for g in &generations {
            if !read_segment(&segment_path(dir, *g), &mut ops)? {
                torn_segments += 1;
            }
        }
        let next = generations.last().map_or(1, |g| g + 1);
        let file = create_segment(dir, next)?;
        let log = Self {
            dir: dir.to_path_buf(),
            writer: Mutex::new(Writer {
                file,
                generation: next,
            }),
            gate: RwLock::new(()),
            dirty: AtomicBool::new(false),
            sync_every_append: sync_interval.is_zero(),
            appended: AtomicU64::new(0),
        };
        Ok((
            log,
            Recovered {
                ops,
                segments: generations.len(),
                torn_segments,
            },
        ))
    }

    /// Apply a mutation and log what it produced, as one step with respect to
    /// checkpoints. `apply` returns its result and the records to log (none when the
    /// mutation changed nothing). It must not call back into this log.
    pub fn record<R>(&self, apply: impl FnOnce() -> (R, Vec<Op>)) -> R {
        let _shared = self.gate.read();
        let (result, ops) = apply();
        for op in &ops {
            self.log(op);
        }
        result
    }

    /// Append one record for a mutation whose replay is idempotent (a KV or
    /// document post-image), outside the checkpoint gate. The caller must log under
    /// the same lock it applied the mutation with, so log order matches apply order.
    pub fn log(&self, op: &Op) {
        if let Err(e) = self.append(op) {
            tracing::error!("Redo log append failed: {}", e);
        }
    }

    fn append(&self, op: &Op) -> std::io::Result<()> {
        let payload = bincode::serialize(op).map_err(std::io::Error::other)?;
        let len = u32::try_from(payload.len())
            .ok()
            .filter(|l| *l <= MAX_RECORD_BYTES)
            .ok_or_else(|| std::io::Error::other("redo record too large"))?;
        let mut frame = Vec::with_capacity(8 + payload.len());
        frame.extend_from_slice(&len.to_le_bytes());
        frame.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
        frame.extend_from_slice(&payload);
        let mut w = self.writer.lock();
        w.file.write_all(&frame)?;
        if self.sync_every_append {
            w.file.sync_data()?;
        } else {
            self.dirty.store(true, Ordering::Release);
        }
        self.appended.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// `fsync` the current segment if anything was appended since the last sync.
    pub fn sync(&self) -> std::io::Result<()> {
        if self.dirty.swap(false, Ordering::AcqRel) {
            self.writer.lock().file.sync_data()?;
        }
        Ok(())
    }

    /// Start a checkpoint: with no mutation in flight, rotate to a new generation and
    /// run `capture`. Returns the last generation the captured state covers (pass it
    /// to [`OpLog::retire`] once that state is durable) and the capture's result.
    pub fn cut<T>(&self, capture: impl FnOnce() -> T) -> std::io::Result<(u64, T)> {
        let _exclusive = self.gate.write();
        let covered = {
            let mut w = self.writer.lock();
            w.file.sync_data()?;
            let covered = w.generation;
            w.file = create_segment(&self.dir, covered + 1)?;
            w.generation = covered + 1;
            covered
        };
        self.dirty.store(false, Ordering::Release);
        Ok((covered, capture()))
    }

    /// Delete every generation up to and including `covered`.
    pub fn retire(&self, covered: u64) -> std::io::Result<usize> {
        let mut removed = 0;
        for g in list_generations(&self.dir)? {
            if g <= covered {
                std::fs::remove_file(segment_path(&self.dir, g))?;
                removed += 1;
            }
        }
        sync_dir(&self.dir);
        Ok(removed)
    }

    /// Records appended since the log was opened.
    pub fn appended(&self) -> u64 {
        self.appended.load(Ordering::Relaxed)
    }
}

fn segment_path(dir: &Path, generation: u64) -> PathBuf {
    dir.join(format!("{:016}.log", generation))
}

fn create_segment(dir: &Path, generation: u64) -> std::io::Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(segment_path(dir, generation))?;
    sync_dir(dir);
    Ok(file)
}

fn list_generations(dir: &Path) -> std::io::Result<Vec<u64>> {
    let mut gens: Vec<u64> = std::fs::read_dir(dir)?
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".log"))
                .and_then(|n| n.parse().ok())
        })
        .collect();
    gens.sort_unstable();
    Ok(gens)
}

/// Read one segment's records into `ops`. Returns false when the segment ended in
/// a torn or corrupt frame (the records before it are kept).
fn read_segment(path: &Path, ops: &mut Vec<Op>) -> std::io::Result<bool> {
    let mut bytes = Vec::new();
    File::open(path)?.read_to_end(&mut bytes)?;
    let mut at = 0usize;
    while at < bytes.len() {
        let Some(header) = bytes.get(at..at + 8) else {
            return Ok(false);
        };
        let len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let crc = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        if len > MAX_RECORD_BYTES {
            return Ok(false);
        }
        let Some(payload) = bytes.get(at + 8..at + 8 + len as usize) else {
            return Ok(false);
        };
        if crc32fast::hash(payload) != crc {
            return Ok(false);
        }
        match bincode::deserialize::<Op>(payload) {
            Ok(op) => ops.push(op),
            Err(_) => return Ok(false),
        }
        at += 8 + len as usize;
    }
    Ok(true)
}

/// Make a create / rename / remove in `dir` durable. Best effort: some filesystems
/// refuse to open a directory for sync.
pub fn sync_dir(dir: &Path) {
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
}

/// Write `bytes` to `path` so a crash leaves either the old file or the new one,
/// never a torn mix: write a sibling temp file, `fsync` it, rename it over `path`,
/// then `fsync` the directory.
pub fn write_atomic(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    let (path, bytes) = (path.as_ref(), bytes.as_ref());
    let tmp = path.with_extension(match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{ext}.tmp"),
        None => "tmp".to_string(),
    });
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Some(parent) = path.parent() {
        sync_dir(parent);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(key: &str) -> Op {
        Op::KvPut {
            key: key.to_string(),
            value_json: "1".to_string(),
            ttl: None,
            created_ns: 1,
            updated_ns: 2,
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("aegis-oplog-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn records_survive_reopen_in_order() {
        let dir = temp_dir("reopen");
        {
            let (log, rec) = OpLog::open(&dir, Duration::from_secs(1)).expect("open");
            assert!(rec.ops.is_empty());
            log.record(|| ((), vec![put("a"), put("b")]));
            log.record(|| ((), vec![Op::KvDelete { key: "a".into() }]));
            // dropped without a sync: the records are already in the file
        }
        let (_log, rec) = OpLog::open(&dir, Duration::from_secs(1)).expect("reopen");
        assert_eq!(
            rec.ops,
            vec![put("a"), put("b"), Op::KvDelete { key: "a".into() }]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn torn_tail_keeps_the_records_before_it() {
        let dir = temp_dir("torn");
        {
            let (log, _) = OpLog::open(&dir, Duration::ZERO).expect("open");
            log.record(|| ((), vec![put("a"), put("b")]));
        }
        let seg = segment_path(&dir, 1);
        let mut bytes = std::fs::read(&seg).expect("read");
        let keep = bytes.len() - 3;
        bytes.truncate(keep);
        std::fs::write(&seg, &bytes).expect("write");
        let (_log, rec) = OpLog::open(&dir, Duration::ZERO).expect("reopen");
        assert_eq!(rec.ops, vec![put("a")]);
        assert_eq!(rec.torn_segments, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cut_and_retire_drop_only_covered_generations() {
        let dir = temp_dir("cut");
        let (log, _) = OpLog::open(&dir, Duration::from_secs(1)).expect("open");
        log.record(|| ((), vec![put("before")]));
        let (covered, captured) = log.cut(|| 7).expect("cut");
        assert_eq!(captured, 7);
        log.record(|| ((), vec![put("after")]));
        assert_eq!(log.retire(covered).expect("retire"), 1);
        drop(log);
        let (_log, rec) = OpLog::open(&dir, Duration::from_secs(1)).expect("reopen");
        assert_eq!(rec.ops, vec![put("after")]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_replaces_the_file() {
        let dir = temp_dir("atomic");
        std::fs::create_dir_all(&dir).expect("dir");
        let p = dir.join("state.json");
        write_atomic(&p, b"one").expect("first");
        write_atomic(&p, b"two").expect("second");
        assert_eq!(std::fs::read(&p).expect("read"), b"two");
        assert!(!dir.join("state.json.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
