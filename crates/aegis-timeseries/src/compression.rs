//! Aegis Time Series Compression
//!
//! Gorilla-style compression for time series data using delta-of-delta
//! encoding for timestamps and XOR-based encoding for values.
//!
//! Cold Gorilla blocks can be recompressed with NexusCompress for a higher ratio
//! via the admin compress endpoint. The on-disk bincode format is positional, so
//! adding a field to `CompressedBlock` / `PersistedBlock` would break loading of
//! every existing `timeseries.bin`. The codec is therefore detected from the
//! block bytes themselves: a NexusCompress frame always begins with the `NCZL`
//! magic, while a Gorilla bitstream begins with the high byte of the first (ms)
//! timestamp, which is `0x00` for any realistic timestamp. The two are
//! unambiguous, and reads dispatch transparently through `decode_block`.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::types::DataPoint;
use chrono::DateTime;
#[cfg(test)]
use chrono::Utc;
use nexuscompress::core_types::{DType, Domain, Schema};
use nexuscompress::timeseries::TimeSeriesCompressor;

/// Mask for the 6-bit "meaningful bits" field of a Gorilla value header.
const MEANINGFUL_BITS_MASK: u8 = 0x3f;

// =============================================================================
// Bit Writer
// =============================================================================

/// Bit-level writer for compression.
struct BitWriter {
    buffer: Vec<u8>,
    current_byte: u8,
    bit_position: u8,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            buffer: Vec::new(),
            current_byte: 0,
            bit_position: 0,
        }
    }

    fn write_bit(&mut self, bit: bool) {
        if bit {
            self.current_byte |= 1 << (7 - self.bit_position);
        }
        self.bit_position += 1;

        if self.bit_position == 8 {
            self.buffer.push(self.current_byte);
            self.current_byte = 0;
            self.bit_position = 0;
        }
    }

    fn write_bits(&mut self, value: u64, num_bits: u8) {
        for i in (0..num_bits).rev() {
            self.write_bit((value >> i) & 1 == 1);
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bit_position > 0 {
            self.buffer.push(self.current_byte);
        }
        self.buffer
    }
}

// =============================================================================
// Bit Reader
// =============================================================================

/// Bit-level reader for decompression.
struct BitReader<'a> {
    data: &'a [u8],
    byte_position: usize,
    bit_position: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_position: 0,
            bit_position: 0,
        }
    }

    fn read_bit(&mut self) -> Option<bool> {
        if self.byte_position >= self.data.len() {
            return None;
        }

        let bit = (self.data[self.byte_position] >> (7 - self.bit_position)) & 1 == 1;
        self.bit_position += 1;

        if self.bit_position == 8 {
            self.byte_position += 1;
            self.bit_position = 0;
        }

        Some(bit)
    }

    fn read_bits(&mut self, num_bits: u8) -> Option<u64> {
        let mut value = 0u64;
        for _ in 0..num_bits {
            value = (value << 1) | (self.read_bit()? as u64);
        }
        Some(value)
    }
}

// =============================================================================
// Compressor
// =============================================================================

/// Gorilla-style time series compressor.
pub struct Compressor {
    writer: BitWriter,
    first_timestamp: Option<i64>,
    prev_timestamp: i64,
    prev_delta: i64,
    prev_value_bits: u64,
    prev_leading_zeros: u8,
    prev_trailing_zeros: u8,
    count: usize,
}

impl Compressor {
    pub fn new() -> Self {
        Self {
            writer: BitWriter::new(),
            first_timestamp: None,
            prev_timestamp: 0,
            prev_delta: 0,
            prev_value_bits: 0,
            prev_leading_zeros: 64,
            prev_trailing_zeros: 64,
            count: 0,
        }
    }

    /// Compress a data point.
    pub fn compress(&mut self, point: &DataPoint) {
        let timestamp = point.timestamp_millis();
        let value_bits = point.value.to_bits();

        if self.first_timestamp.is_none() {
            self.first_timestamp = Some(timestamp);
            self.writer.write_bits(timestamp as u64, 64);
            self.writer.write_bits(value_bits, 64);
            self.prev_timestamp = timestamp;
            self.prev_value_bits = value_bits;
            self.count = 1;
            return;
        }

        self.compress_timestamp(timestamp);
        self.compress_value(value_bits);

        self.prev_timestamp = timestamp;
        self.prev_value_bits = value_bits;
        self.count += 1;
    }

    fn compress_timestamp(&mut self, timestamp: i64) {
        let delta = timestamp - self.prev_timestamp;
        let delta_of_delta = delta - self.prev_delta;

        if delta_of_delta == 0 {
            self.writer.write_bit(false);
        } else if (-63..=64).contains(&delta_of_delta) {
            self.writer.write_bits(0b10, 2);
            self.writer.write_bits((delta_of_delta + 63) as u64, 7);
        } else if (-255..=256).contains(&delta_of_delta) {
            self.writer.write_bits(0b110, 3);
            self.writer.write_bits((delta_of_delta + 255) as u64, 9);
        } else if (-2047..=2048).contains(&delta_of_delta) {
            self.writer.write_bits(0b1110, 4);
            self.writer.write_bits((delta_of_delta + 2047) as u64, 12);
        } else {
            self.writer.write_bits(0b1111, 4);
            self.writer.write_bits(delta_of_delta as u64, 64);
        }

        self.prev_delta = delta;
    }

    fn compress_value(&mut self, value_bits: u64) {
        let xor = self.prev_value_bits ^ value_bits;

        if xor == 0 {
            self.writer.write_bit(false);
            return;
        }

        self.writer.write_bit(true);

        let leading_zeros = xor.leading_zeros() as u8;
        let trailing_zeros = xor.trailing_zeros() as u8;

        if leading_zeros >= self.prev_leading_zeros && trailing_zeros >= self.prev_trailing_zeros {
            self.writer.write_bit(false);
            let meaningful_bits = 64 - self.prev_leading_zeros - self.prev_trailing_zeros;
            let shifted = xor >> self.prev_trailing_zeros;
            self.writer.write_bits(shifted, meaningful_bits);
        } else {
            self.writer.write_bit(true);
            self.writer.write_bits(leading_zeros as u64, 6);

            // `meaningful_bits` is 1..=64 but the field is 6 bits wide, so 64
            // is stored as 0 (the standard Gorilla convention). The decoder
            // maps 0 back to 64. This is the same bit pattern older encoders
            // produced, so existing blocks decode correctly.
            let meaningful_bits = 64 - leading_zeros - trailing_zeros;
            self.writer
                .write_bits(u64::from(meaningful_bits & MEANINGFUL_BITS_MASK), 6);

            let shifted = xor >> trailing_zeros;
            self.writer.write_bits(shifted, meaningful_bits);

            self.prev_leading_zeros = leading_zeros;
            self.prev_trailing_zeros = trailing_zeros;
        }
    }

    /// Finish compression and return the compressed data.
    pub fn finish(self) -> CompressedBlock {
        let data = self.writer.finish();
        let checksum = crc32fast::hash(&data);
        CompressedBlock {
            data,
            first_timestamp: self.first_timestamp.unwrap_or(0),
            last_timestamp: self.prev_timestamp,
            count: self.count,
            checksum,
        }
    }
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Compressed Block
// =============================================================================

/// A compressed block of time series data.
#[derive(Debug, Clone)]
pub struct CompressedBlock {
    pub data: Vec<u8>,
    pub first_timestamp: i64,
    pub last_timestamp: i64,
    pub count: usize,
    pub checksum: u32,
}

impl CompressedBlock {
    /// Get compression ratio.
    pub fn compression_ratio(&self) -> f64 {
        let uncompressed_size = self.count * 16;
        if self.data.is_empty() {
            return 1.0;
        }
        uncompressed_size as f64 / self.data.len() as f64
    }

    /// Verify block integrity using CRC32 checksum.
    pub fn verify_checksum(&self) -> bool {
        crc32fast::hash(&self.data) == self.checksum
    }
}

// =============================================================================
// Decompressor
// =============================================================================

/// Gorilla-style time series decompressor.
#[allow(dead_code)]
pub struct Decompressor<'a> {
    reader: BitReader<'a>,
    first_timestamp: i64,
    prev_timestamp: i64,
    prev_delta: i64,
    prev_value_bits: u64,
    prev_leading_zeros: u8,
    prev_trailing_zeros: u8,
    remaining: usize,
    first_read: bool,
}

impl<'a> Decompressor<'a> {
    pub fn new(block: &'a CompressedBlock) -> Self {
        Self {
            reader: BitReader::new(&block.data),
            first_timestamp: block.first_timestamp,
            prev_timestamp: 0,
            prev_delta: 0,
            prev_value_bits: 0,
            prev_leading_zeros: 0,
            prev_trailing_zeros: 0,
            remaining: block.count,
            first_read: true,
        }
    }

    /// Decompress the next data point.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<DataPoint> {
        if self.remaining == 0 {
            return None;
        }

        self.remaining -= 1;

        if self.first_read {
            self.first_read = false;
            let timestamp = self.reader.read_bits(64)? as i64;
            let value_bits = self.reader.read_bits(64)?;
            self.prev_timestamp = timestamp;
            self.prev_value_bits = value_bits;

            return Some(DataPoint {
                timestamp: DateTime::from_timestamp_millis(timestamp)?,
                value: f64::from_bits(value_bits),
            });
        }

        let timestamp = self.decompress_timestamp()?;
        let value_bits = self.decompress_value()?;

        self.prev_timestamp = timestamp;
        self.prev_value_bits = value_bits;

        Some(DataPoint {
            timestamp: DateTime::from_timestamp_millis(timestamp)?,
            value: f64::from_bits(value_bits),
        })
    }

    fn decompress_timestamp(&mut self) -> Option<i64> {
        let delta_of_delta = if !self.reader.read_bit()? {
            0i64
        } else if !self.reader.read_bit()? {
            self.reader.read_bits(7)? as i64 - 63
        } else if !self.reader.read_bit()? {
            self.reader.read_bits(9)? as i64 - 255
        } else if !self.reader.read_bit()? {
            self.reader.read_bits(12)? as i64 - 2047
        } else {
            self.reader.read_bits(64)? as i64
        };

        self.prev_delta = self.prev_delta.checked_add(delta_of_delta)?;
        self.prev_timestamp.checked_add(self.prev_delta)
    }

    fn decompress_value(&mut self) -> Option<u64> {
        if !self.reader.read_bit()? {
            return Some(self.prev_value_bits);
        }

        let xor = if !self.reader.read_bit()? {
            let meaningful_bits = 64u8
                .checked_sub(self.prev_leading_zeros)?
                .checked_sub(self.prev_trailing_zeros)?;
            if meaningful_bits == 0 {
                // A reuse marker before any header: corrupt stream.
                return None;
            }
            let value = self.reader.read_bits(meaningful_bits)?;
            value.checked_shl(u32::from(self.prev_trailing_zeros))?
        } else {
            let leading_zeros = self.reader.read_bits(6)? as u8;
            let mut meaningful_bits = self.reader.read_bits(6)? as u8;
            if meaningful_bits == 0 {
                // 64 meaningful bits do not fit in the 6-bit field and are
                // stored as 0 (see `compress_value`).
                meaningful_bits = 64;
            }
            let trailing_zeros = 64u8
                .checked_sub(leading_zeros)?
                .checked_sub(meaningful_bits)?;

            self.prev_leading_zeros = leading_zeros;
            self.prev_trailing_zeros = trailing_zeros;

            let value = self.reader.read_bits(meaningful_bits)?;
            value.checked_shl(u32::from(trailing_zeros))?
        };

        Some(self.prev_value_bits ^ xor)
    }

    /// Decompress all points.
    pub fn decompress_all(&mut self) -> Vec<DataPoint> {
        let mut points = Vec::with_capacity(self.remaining);
        while let Some(point) = self.next() {
            points.push(point);
        }
        points
    }
}

// =============================================================================
// NexusCompress codec (high-ratio cold-block recompression)
// =============================================================================

/// Leading bytes of a serialized NexusCompress frame (`NCZL`).
const NCZL_MAGIC: [u8; 4] = [0x4E, 0x43, 0x5A, 0x4C];

/// Columnar schema for an Aegis time-series block: ms timestamp + value.
fn nexus_ts_schema() -> Schema {
    Schema::builder()
        .type_name("AegisSeriesBlock")
        .domain(Domain::TimeSeries)
        .field("timestamp", DType::I64)
        .field("value", DType::F64)
        .build()
}

/// True if a block's bytes are a NexusCompress frame (vs a Gorilla bitstream).
pub fn is_nexus_block(block: &CompressedBlock) -> bool {
    block.data.len() >= 4 && block.data[0..4] == NCZL_MAGIC
}

/// Re-encode points as a NexusCompress-backed `CompressedBlock`. Returns `None`
/// if there are no points or compression fails, in which case the caller keeps
/// the original block (fail-safe — never loses data).
pub fn compress_points_nexus(points: &[DataPoint]) -> Option<CompressedBlock> {
    if points.is_empty() {
        return None;
    }
    let mut ts_bytes = Vec::with_capacity(points.len() * 8);
    let mut val_bytes = Vec::with_capacity(points.len() * 8);
    for p in points {
        ts_bytes.extend_from_slice(&p.timestamp_millis().to_le_bytes());
        val_bytes.extend_from_slice(&p.value.to_le_bytes());
    }
    let compressor = TimeSeriesCompressor::from_schema(nexus_ts_schema());
    let frame = compressor
        .compress_batch(&[ts_bytes, val_bytes], points.len() as u64)
        .ok()?;
    if frame.len() < 4 || frame[0..4] != NCZL_MAGIC {
        return None;
    }
    let first_ts = points
        .iter()
        .map(|p| p.timestamp_millis())
        .min()
        .unwrap_or(0);
    let last_ts = points
        .iter()
        .map(|p| p.timestamp_millis())
        .max()
        .unwrap_or(0);
    let checksum = crc32fast::hash(&frame);
    Some(CompressedBlock {
        data: frame,
        first_timestamp: first_ts,
        last_timestamp: last_ts,
        count: points.len(),
        checksum,
    })
}

/// True if `block` decodes to exactly `points` (same count, timestamps and
/// value bit patterns). Used before a freshly encoded block replaces the data it
/// was built from, so an encoder fault can never silently lose points.
pub fn block_matches_points(block: &CompressedBlock, points: &[DataPoint]) -> bool {
    if block.count != points.len() {
        return false;
    }
    let decoded = decode_block(block);
    decoded.len() == points.len()
        && decoded.iter().zip(points).all(|(d, p)| {
            d.timestamp.timestamp_millis() == p.timestamp.timestamp_millis()
                && d.value.to_bits() == p.value.to_bits()
        })
}

/// Decode a block's points, dispatching on the embedded codec (Gorilla vs
/// NexusCompress). This is the single transparent-read hook used by queries.
pub fn decode_block(block: &CompressedBlock) -> Vec<DataPoint> {
    if !is_nexus_block(block) {
        return Decompressor::new(block).decompress_all();
    }
    let compressor = TimeSeriesCompressor::from_schema(nexus_ts_schema());
    let cols = match compressor.decompress_batch(&block.data) {
        Ok(c) if c.len() == 2 => c,
        _ => return Vec::new(),
    };
    let (ts_col, val_col) = (&cols[0], &cols[1]);
    let n = ts_col.len() / 8;
    let mut points = Vec::with_capacity(n);
    for i in 0..n {
        let off = i * 8;
        let ts = match ts_col.get(off..off + 8).and_then(|s| s.try_into().ok()) {
            Some(b) => i64::from_le_bytes(b),
            None => continue,
        };
        let value = match val_col.get(off..off + 8).and_then(|s| s.try_into().ok()) {
            Some(b) => f64::from_le_bytes(b),
            None => 0.0,
        };
        if let Some(timestamp) = DateTime::from_timestamp_millis(ts) {
            points.push(DataPoint { timestamp, value });
        }
    }
    points
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_compress_decompress() {
        let mut compressor = Compressor::new();

        let base_time = Utc::now();
        let points: Vec<DataPoint> = (0..100)
            .map(|i| DataPoint {
                timestamp: base_time + Duration::seconds(i),
                value: 42.0 + (i as f64 * 0.1),
            })
            .collect();

        for point in &points {
            compressor.compress(point);
        }

        let block = compressor.finish();
        assert!(block.compression_ratio() > 1.0);

        let mut decompressor = Decompressor::new(&block);
        let decompressed = decompressor.decompress_all();

        assert_eq!(decompressed.len(), points.len());
        for (original, decoded) in points.iter().zip(decompressed.iter()) {
            assert_eq!(original.value, decoded.value);
        }
    }

    #[test]
    fn test_compression_ratio() {
        let mut compressor = Compressor::new();

        let base_time = Utc::now();
        for i in 0..1000 {
            compressor.compress(&DataPoint {
                timestamp: base_time + Duration::seconds(i),
                value: 100.0 + (i as f64 % 10.0),
            });
        }

        let block = compressor.finish();
        let ratio = block.compression_ratio();

        assert!(ratio > 2.0, "Expected compression ratio > 2, got {}", ratio);
    }

    fn gorilla(points: &[DataPoint]) -> CompressedBlock {
        let mut c = Compressor::new();
        for p in points {
            c.compress(p);
        }
        c.finish()
    }

    fn assert_roundtrip(points: &[DataPoint]) {
        let block = gorilla(points);
        let decoded = Decompressor::new(&block).decompress_all();
        assert_eq!(decoded.len(), points.len(), "point count");
        for (i, (o, d)) in points.iter().zip(decoded.iter()).enumerate() {
            assert_eq!(
                o.timestamp.timestamp_millis(),
                d.timestamp.timestamp_millis(),
                "timestamp {i}"
            );
            assert_eq!(o.value.to_bits(), d.value.to_bits(), "value {i}");
        }
        assert!(block_matches_points(&block, points));
    }

    fn series(values: &[f64]) -> Vec<DataPoint> {
        let base = DateTime::from_timestamp_millis(1_760_000_000_000).unwrap();
        values
            .iter()
            .enumerate()
            .map(|(i, v)| DataPoint {
                timestamp: base + Duration::seconds(i as i64 * 30),
                value: *v,
            })
            .collect()
    }

    /// A sign flip makes the XOR's top bit set and (for these values) its low
    /// bit set too: 64 meaningful bits, which the 6-bit header stores as 0.
    /// Before the fix the decoder treated 0 as 0 bits and corrupted every
    /// following point of the block.
    #[test]
    fn test_sign_flip_roundtrip() {
        assert_roundtrip(&series(&[
            21.0, 21.5, -0.3, 21.5, 0.1, -17.25, 3.0e-300, -1.0,
        ]));
        assert_roundtrip(&series(&[1.0, f64::from_bits(1.0f64.to_bits() ^ u64::MAX)]));
        assert_roundtrip(&series(&[
            0.0,
            -0.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ]));
    }

    /// Blocks written by the old encoder carry the same bits (64 stored as 0),
    /// so the fixed decoder repairs them without a migration.
    #[test]
    fn test_full_width_xor_header_is_zero() {
        let pts = series(&[21.0, -0.3]);
        let block = gorilla(&pts);
        // First point raw, then the second timestamp, then the value header:
        // '1' (changed), '1' (new window), 6 bits lz, 6 bits meaningful.
        let mut d = Decompressor::new(&block);
        d.next().unwrap();
        d.decompress_timestamp().unwrap();
        let r = &mut d.reader;
        assert!(r.read_bit().unwrap());
        assert!(r.read_bit().unwrap());
        assert_eq!(r.read_bits(6).unwrap(), 0);
        assert_eq!(r.read_bits(6).unwrap(), 0);
        assert_eq!(decode_block(&block).len(), 2);
    }

    #[test]
    fn test_corrupt_block_does_not_panic() {
        let block = gorilla(&series(&[21.0, -0.3, 5.0, 6.0]));
        for cut in 0..block.data.len() {
            let mut b = block.clone();
            b.data.truncate(cut);
            let out = Decompressor::new(&b).decompress_all();
            assert!(out.len() <= 4);
        }
        let mut b = block.clone();
        for byte in b.data.iter_mut().skip(16) {
            *byte = 0xff;
        }
        let _ = Decompressor::new(&b).decompress_all();
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]
        #[test]
        fn prop_gorilla_roundtrip(
            bits in proptest::collection::vec(proptest::prelude::any::<u64>(), 1..200),
            steps in proptest::collection::vec(0i64..5_000_000, 1..200),
        ) {
            let base = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
            let mut ts = base;
            let points: Vec<DataPoint> = bits
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    ts += Duration::milliseconds(steps[i % steps.len()]);
                    DataPoint { timestamp: ts, value: f64::from_bits(*b) }
                })
                .collect();
            assert_roundtrip(&points);
        }

        #[test]
        fn prop_gorilla_roundtrip_signed_small(
            vals in proptest::collection::vec(-100.0f64..100.0, 1..300),
        ) {
            assert_roundtrip(&series(&vals));
        }
    }

    #[test]
    fn test_nexus_roundtrip_and_detection() {
        let base_time = Utc::now();
        let points: Vec<DataPoint> = (0..500)
            .map(|i| DataPoint {
                timestamp: base_time + Duration::seconds(i),
                value: 70.0 + (i as f64 % 5.0),
            })
            .collect();

        let mut g = Compressor::new();
        for p in &points {
            g.compress(p);
        }
        let gblock = g.finish();
        assert!(!is_nexus_block(&gblock));

        let nblock = compress_points_nexus(&points).expect("nexus block");
        assert!(is_nexus_block(&nblock));
        assert!(nblock.verify_checksum());
        assert_eq!(nblock.count, points.len());

        let from_gorilla = decode_block(&gblock);
        let from_nexus = decode_block(&nblock);
        assert_eq!(from_gorilla.len(), points.len());
        assert_eq!(from_nexus.len(), points.len());
        for (orig, dec) in points.iter().zip(from_nexus.iter()) {
            assert_eq!(
                orig.timestamp.timestamp_millis(),
                dec.timestamp.timestamp_millis()
            );
            assert_eq!(orig.value, dec.value);
        }
    }
}
