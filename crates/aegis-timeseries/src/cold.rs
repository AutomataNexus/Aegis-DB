//! Aegis Time Series Cold Tier
//!
//! Long-term, on-disk storage for compressed blocks that have aged out of the
//! resident ("hot") series map. The hot map is bounded by a short retention
//! window so RAM stays flat; instead of discarding the evicted blocks, the engine
//! hands them to the [`ColdStore`], which appends them to one append-only file per
//! series. Queries whose range reaches past the hot window read the matching
//! frames back (skipping by the per-frame timestamp header, so only the blocks in
//! range are decoded) and merge them with the hot data.
//!
//! On-disk layout (`<data_path>/cold/`):
//! - `series.idx` — bincode `HashMap<series_id, ColdSeriesMeta>` so series that
//!   no longer exist in the hot map are still discoverable.
//! - `<fnv64>.act` — append-only frames for one series (name = FNV-1a of the id).
//!
//! Frame: `magic(4) | first_ts(i64) | last_ts(i64) | count(u64) | checksum(u32) |
//! len(u32) | data[len]`. Blocks are stored exactly as they live in memory
//! (Gorilla-compressed), so appending is a copy and reading needs no re-encode.
//!
//! Cold retention is enforced by [`ColdStore::compact`], which rewrites each file
//! without the frames older than the cold cutoff (and removes empty files).
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::compression::CompressedBlock;
use crate::types::{Metric, Tags};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const FRAME_MAGIC: [u8; 4] = *b"ACT1";
const FRAME_HEADER_LEN: u64 = 4 + 8 + 8 + 8 + 4 + 4;
const INDEX_FILE: &str = "series.idx";

/// What we need to rebuild a `Series` for a cold-only series id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColdSeriesMeta {
    pub metric: Metric,
    pub tags: Tags,
}

/// Outcome of a compaction pass.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ColdCompactReport {
    pub files_scanned: usize,
    pub files_rewritten: usize,
    pub files_removed: usize,
    pub frames_dropped: usize,
}

/// Per-series append-only cold storage.
pub struct ColdStore {
    dir: PathBuf,
    index: RwLock<HashMap<String, ColdSeriesMeta>>,
}

impl ColdStore {
    /// Open (or create) the cold store under `dir`, loading its series index.
    pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        let index = match File::open(dir.join(INDEX_FILE)) {
            Ok(f) => bincode::deserialize_from(BufReader::new(f)).unwrap_or_else(|e| {
                eprintln!("cold tier: series index unreadable ({e}); starting empty");
                HashMap::new()
            }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => return Err(e),
        };
        Ok(Self {
            dir,
            index: RwLock::new(index),
        })
    }

    /// Every series the cold tier knows about (for re-registering in the index).
    pub fn series(&self) -> Vec<(String, ColdSeriesMeta)> {
        self.index
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Whether this series has any cold data.
    pub fn contains(&self, series_id: &str) -> bool {
        self.index.read().contains_key(series_id)
    }

    /// Number of series with cold data.
    pub fn len(&self) -> usize {
        self.index.read().len()
    }

    /// Whether the cold tier is empty.
    pub fn is_empty(&self) -> bool {
        self.index.read().is_empty()
    }

    fn file_for(&self, series_id: &str) -> PathBuf {
        self.dir.join(format!("{:016x}.act", fnv1a64(series_id)))
    }

    fn save_index(&self) -> io::Result<()> {
        let tmp = self.dir.join(format!("{INDEX_FILE}.tmp"));
        {
            let f = File::create(&tmp)?;
            let mut w = BufWriter::new(f);
            bincode::serialize_into(&mut w, &*self.index.read())
                .map_err(io::Error::other)?;
            w.flush()?;
        }
        fs::rename(tmp, self.dir.join(INDEX_FILE))
    }

    /// Append evicted blocks for a series. Blocks are written in the order given.
    pub fn append(
        &self,
        series_id: &str,
        metric: &Metric,
        tags: &Tags,
        blocks: &[CompressedBlock],
    ) -> io::Result<()> {
        if blocks.is_empty() {
            return Ok(());
        }
        let path = self.file_for(series_id);
        let mut w = BufWriter::new(OpenOptions::new().create(true).append(true).open(&path)?);
        for b in blocks {
            write_frame(&mut w, b)?;
        }
        w.flush()?;

        let is_new = {
            let mut idx = self.index.write();
            idx.insert(
                series_id.to_string(),
                ColdSeriesMeta {
                    metric: metric.clone(),
                    tags: tags.clone(),
                },
            )
            .is_none()
        };
        if is_new {
            self.save_index()?;
        }
        Ok(())
    }

    /// Read every block of a series that overlaps `[start_ms, end_ms]`, skipping the
    /// payload of frames outside the range.
    pub fn read_range(&self, series_id: &str, start_ms: i64, end_ms: i64) -> Vec<CompressedBlock> {
        if !self.contains(series_id) {
            return Vec::new();
        }
        let path = self.file_for(series_id);
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let mut r = BufReader::new(file);
        let mut out = Vec::new();
        loop {
            match read_frame_header(&mut r) {
                Ok(Some(h)) => {
                    if h.last_ts < start_ms || h.first_ts > end_ms {
                        if r.seek(SeekFrom::Current(h.len as i64)).is_err() {
                            break;
                        }
                        continue;
                    }
                    let mut data = vec![0u8; h.len as usize];
                    if r.read_exact(&mut data).is_err() {
                        break;
                    }
                    out.push(CompressedBlock {
                        data,
                        first_timestamp: h.first_ts,
                        last_timestamp: h.last_ts,
                        count: h.count as usize,
                        checksum: h.checksum,
                    });
                }
                Ok(None) => break,
                Err(_) => break, // truncated tail (crash mid-append): ignore the remainder
            }
        }
        out
    }

    /// Drop every frame whose newest point is older than `cutoff_ms`, rewriting the
    /// affected files and deleting the ones left empty.
    pub fn compact(&self, cutoff_ms: i64) -> ColdCompactReport {
        let mut report = ColdCompactReport::default();
        let ids: Vec<String> = self.index.read().keys().cloned().collect();
        let mut removed_ids = Vec::new();

        for id in ids {
            let path = self.file_for(&id);
            report.files_scanned += 1;
            let Ok(file) = File::open(&path) else {
                removed_ids.push(id);
                continue;
            };
            let mut r = BufReader::new(file);
            let mut keep: Vec<CompressedBlock> = Vec::new();
            let mut dropped = 0usize;
            loop {
                match read_frame_header(&mut r) {
                    Ok(Some(h)) => {
                        if h.last_ts < cutoff_ms {
                            dropped += 1;
                            if r.seek(SeekFrom::Current(h.len as i64)).is_err() {
                                break;
                            }
                            continue;
                        }
                        let mut data = vec![0u8; h.len as usize];
                        if r.read_exact(&mut data).is_err() {
                            break;
                        }
                        keep.push(CompressedBlock {
                            data,
                            first_timestamp: h.first_ts,
                            last_timestamp: h.last_ts,
                            count: h.count as usize,
                            checksum: h.checksum,
                        });
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
            if dropped == 0 {
                continue;
            }
            report.frames_dropped += dropped;
            if keep.is_empty() {
                let _ = fs::remove_file(&path);
                report.files_removed += 1;
                removed_ids.push(id);
            } else if rewrite_file(&path, &keep).is_ok() {
                report.files_rewritten += 1;
            }
        }

        if !removed_ids.is_empty() {
            {
                let mut idx = self.index.write();
                for id in &removed_ids {
                    idx.remove(id);
                }
            }
            let _ = self.save_index();
        }
        report
    }

    /// Total bytes on disk (for stats/admin).
    pub fn disk_bytes(&self) -> u64 {
        fs::read_dir(&self.dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter_map(|e| e.metadata().ok())
                    .map(|m| m.len())
                    .sum()
            })
            .unwrap_or(0)
    }

    /// Directory backing this store.
    pub fn path(&self) -> &Path {
        &self.dir
    }
}

struct FrameHeader {
    first_ts: i64,
    last_ts: i64,
    count: u64,
    checksum: u32,
    len: u32,
}

fn write_frame<W: Write>(w: &mut W, b: &CompressedBlock) -> io::Result<()> {
    w.write_all(&FRAME_MAGIC)?;
    w.write_all(&b.first_timestamp.to_le_bytes())?;
    w.write_all(&b.last_timestamp.to_le_bytes())?;
    w.write_all(&(b.count as u64).to_le_bytes())?;
    w.write_all(&b.checksum.to_le_bytes())?;
    w.write_all(&(b.data.len() as u32).to_le_bytes())?;
    w.write_all(&b.data)
}

/// `Ok(None)` at a clean EOF; `Err` on a truncated/corrupt header.
fn read_frame_header<R: Read>(r: &mut R) -> io::Result<Option<FrameHeader>> {
    let mut buf = [0u8; FRAME_HEADER_LEN as usize];
    match r.read_exact(&mut buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    if buf[0..4] != FRAME_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bad cold frame magic",
        ));
    }
    let i64_at = |o: usize| i64::from_le_bytes(buf[o..o + 8].try_into().unwrap());
    let u64_at = |o: usize| u64::from_le_bytes(buf[o..o + 8].try_into().unwrap());
    let u32_at = |o: usize| u32::from_le_bytes(buf[o..o + 4].try_into().unwrap());
    Ok(Some(FrameHeader {
        first_ts: i64_at(4),
        last_ts: i64_at(12),
        count: u64_at(20),
        checksum: u32_at(28),
        len: u32_at(32),
    }))
}

fn rewrite_file(path: &Path, blocks: &[CompressedBlock]) -> io::Result<()> {
    let tmp = path.with_extension("act.tmp");
    {
        let mut w = BufWriter::new(File::create(&tmp)?);
        for b in blocks {
            write_frame(&mut w, b)?;
        }
        w.flush()?;
    }
    fs::rename(tmp, path)
}

fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compression::{decode_block, Compressor};
    use crate::types::DataPoint;
    use chrono::{TimeZone, Utc};

    fn block(start_s: i64, n: usize) -> CompressedBlock {
        let mut c = Compressor::new();
        for i in 0..n {
            c.compress(&DataPoint {
                timestamp: Utc.timestamp_opt(start_s + i as i64 * 60, 0).unwrap(),
                value: i as f64,
            });
        }
        c.finish()
    }

    #[test]
    fn append_read_compact_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = ColdStore::open(dir.path()).unwrap();
        let metric = Metric::gauge("temp");
        let mut tags = Tags::default();
        tags.0.insert("loc".into(), "a".into());
        let id = "temp:loc=a";

        let b1 = block(1_000_000, 10); // old
        let b2 = block(2_000_000, 10); // newer
        store
            .append(id, &metric, &tags, &[b1.clone(), b2.clone()])
            .unwrap();
        assert!(store.contains(id));

        // Only the second block overlaps this range.
        let got = store.read_range(id, 2_000_000_000, 3_000_000_000);
        assert_eq!(got.len(), 1);
        assert_eq!(decode_block(&got[0]), decode_block(&b2));

        // Everything.
        assert_eq!(store.read_range(id, 0, i64::MAX).len(), 2);

        // Re-open sees the index.
        drop(store);
        let store = ColdStore::open(dir.path()).unwrap();
        assert_eq!(store.len(), 1);

        // Compact away the old block, then everything.
        let r = store.compact(1_500_000_000);
        assert_eq!(r.frames_dropped, 1);
        assert_eq!(store.read_range(id, 0, i64::MAX).len(), 1);
        let r = store.compact(i64::MAX);
        assert_eq!(r.files_removed, 1);
        assert!(!store.contains(id));
        assert!(store.is_empty());
    }

    #[test]
    fn truncated_tail_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let store = ColdStore::open(dir.path()).unwrap();
        let id = "m:k=v";
        store
            .append(id, &Metric::gauge("m"), &Tags::default(), &[block(0, 5)])
            .unwrap();
        // Simulate a crash mid-append: a partial header.
        let path = store.file_for(id);
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"ACT1\x01\x02").unwrap();
        assert_eq!(store.read_range(id, 0, i64::MAX).len(), 1);
    }
}
