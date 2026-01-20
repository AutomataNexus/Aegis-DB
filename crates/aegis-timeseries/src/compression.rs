//! Aegis Time Series Compression
//!
//! Gorilla-style compression for time series data using delta-of-delta
//! encoding for timestamps and XOR-based encoding for values.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::types::DataPoint;
use chrono::{DateTime, Utc};
use std::io::{self, Read, Write};

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
        } else if delta_of_delta >= -63 && delta_of_delta <= 64 {
            self.writer.write_bits(0b10, 2);
            self.writer.write_bits((delta_of_delta + 63) as u64, 7);
        } else if delta_of_delta >= -255 && delta_of_delta <= 256 {
            self.writer.write_bits(0b110, 3);
            self.writer.write_bits((delta_of_delta + 255) as u64, 9);
        } else if delta_of_delta >= -2047 && delta_of_delta <= 2048 {
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

        if leading_zeros >= self.prev_leading_zeros
            && trailing_zeros >= self.prev_trailing_zeros
        {
            self.writer.write_bit(false);
            let meaningful_bits = 64 - self.prev_leading_zeros - self.prev_trailing_zeros;
            let shifted = xor >> self.prev_trailing_zeros;
            self.writer.write_bits(shifted, meaningful_bits);
        } else {
            self.writer.write_bit(true);
            self.writer.write_bits(leading_zeros as u64, 6);

            let meaningful_bits = 64 - leading_zeros - trailing_zeros;
            self.writer.write_bits(meaningful_bits as u64, 6);

            let shifted = xor >> trailing_zeros;
            self.writer.write_bits(shifted, meaningful_bits);

            self.prev_leading_zeros = leading_zeros;
            self.prev_trailing_zeros = trailing_zeros;
        }
    }

    /// Finish compression and return the compressed data.
    pub fn finish(self) -> CompressedBlock {
        CompressedBlock {
            data: self.writer.finish(),
            first_timestamp: self.first_timestamp.unwrap_or(0),
            count: self.count,
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
    pub count: usize,
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
}

// =============================================================================
// Decompressor
// =============================================================================

/// Gorilla-style time series decompressor.
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

        self.prev_delta += delta_of_delta;
        Some(self.prev_timestamp + self.prev_delta)
    }

    fn decompress_value(&mut self) -> Option<u64> {
        if !self.reader.read_bit()? {
            return Some(self.prev_value_bits);
        }

        let xor = if !self.reader.read_bit()? {
            let meaningful_bits = 64 - self.prev_leading_zeros - self.prev_trailing_zeros;
            let value = self.reader.read_bits(meaningful_bits)?;
            value << self.prev_trailing_zeros
        } else {
            let leading_zeros = self.reader.read_bits(6)? as u8;
            let meaningful_bits = self.reader.read_bits(6)? as u8;
            let trailing_zeros = 64 - leading_zeros - meaningful_bits;

            self.prev_leading_zeros = leading_zeros;
            self.prev_trailing_zeros = trailing_zeros;

            let value = self.reader.read_bits(meaningful_bits)?;
            value << trailing_zeros
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
}
