//! Aegis Block - Storage Block Management
//!
//! Defines the fundamental storage unit for the Aegis database. Blocks are
//! the atomic unit of I/O and replication, containing typed data with
//! checksums for integrity verification.
//!
//! Key Features:
//! - Self-describing block headers with type and compression info
//! - CRC32 checksums for corruption detection
//! - Support for multiple block types (table, index, log, metadata)
//! - Efficient serialization with bincode
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use aegis_common::{BlockId, BlockType, CompressionType, EncryptionType, Result, AegisError};
use bytes::{Bytes, BytesMut, BufMut};
use serde::{Deserialize, Serialize};

// =============================================================================
// Constants
// =============================================================================

pub const BLOCK_HEADER_SIZE: usize = 32;
pub const DEFAULT_BLOCK_SIZE: usize = 8192;
pub const MAX_BLOCK_SIZE: usize = 1024 * 1024; // 1 MB

// =============================================================================
// Block Header
// =============================================================================

/// Header containing block metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub block_id: BlockId,
    pub block_type: BlockType,
    pub compression: CompressionType,
    pub encryption: EncryptionType,
    pub data_size: u32,
    pub uncompressed_size: u32,
    pub checksum: u32,
    pub version: u16,
    pub flags: u16,
}

impl BlockHeader {
    pub fn new(block_id: BlockId, block_type: BlockType) -> Self {
        Self {
            block_id,
            block_type,
            compression: CompressionType::None,
            encryption: EncryptionType::None,
            data_size: 0,
            uncompressed_size: 0,
            checksum: 0,
            version: 1,
            flags: 0,
        }
    }

    /// Serialize header to bytes.
    pub fn to_bytes(&self) -> Result<Bytes> {
        bincode::serialize(self)
            .map(Bytes::from)
            .map_err(|e| AegisError::Serialization(e.to_string()))
    }

    /// Deserialize header from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data)
            .map_err(|e| AegisError::Serialization(e.to_string()))
    }
}

// =============================================================================
// Block
// =============================================================================

/// A storage block containing header and data.
#[derive(Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub data: Bytes,
}

impl Block {
    /// Create a new block with the given data.
    pub fn new(block_id: BlockId, block_type: BlockType, data: Bytes) -> Self {
        let checksum = crc32fast::hash(&data);
        let data_size = data.len() as u32;

        let header = BlockHeader {
            block_id,
            block_type,
            compression: CompressionType::None,
            encryption: EncryptionType::None,
            data_size,
            uncompressed_size: data_size,
            checksum,
            version: 1,
            flags: 0,
        };

        Self { header, data }
    }

    /// Create an empty block.
    pub fn empty(block_id: BlockId, block_type: BlockType) -> Self {
        Self::new(block_id, block_type, Bytes::new())
    }

    /// Verify the block's checksum.
    pub fn verify_checksum(&self) -> bool {
        let computed = crc32fast::hash(&self.data);
        computed == self.header.checksum
    }

    /// Update the checksum after modifying data.
    pub fn update_checksum(&mut self) {
        self.header.checksum = crc32fast::hash(&self.data);
        self.header.data_size = self.data.len() as u32;
    }

    /// Compress the block data using the specified algorithm.
    pub fn compress(&mut self, compression: CompressionType) -> Result<()> {
        if self.header.compression != CompressionType::None {
            return Ok(());
        }

        let compressed = match compression {
            CompressionType::None => return Ok(()),
            CompressionType::Lz4 => {
                lz4_flex::compress_prepend_size(&self.data)
            }
            CompressionType::Zstd => {
                zstd::encode_all(self.data.as_ref(), 3)
                    .map_err(|e| AegisError::Storage(e.to_string()))?
            }
            CompressionType::Snappy => {
                let mut encoder = snap::raw::Encoder::new();
                encoder.compress_vec(&self.data)
                    .map_err(|e| AegisError::Storage(e.to_string()))?
            }
        };

        self.header.uncompressed_size = self.header.data_size;
        self.data = Bytes::from(compressed);
        self.header.data_size = self.data.len() as u32;
        self.header.compression = compression;
        self.update_checksum();

        Ok(())
    }

    /// Decompress the block data.
    pub fn decompress(&mut self) -> Result<()> {
        if self.header.compression == CompressionType::None {
            return Ok(());
        }

        let decompressed = match self.header.compression {
            CompressionType::None => return Ok(()),
            CompressionType::Lz4 => {
                lz4_flex::decompress_size_prepended(&self.data)
                    .map_err(|e| AegisError::Storage(e.to_string()))?
            }
            CompressionType::Zstd => {
                zstd::decode_all(self.data.as_ref())
                    .map_err(|e| AegisError::Storage(e.to_string()))?
            }
            CompressionType::Snappy => {
                let mut decoder = snap::raw::Decoder::new();
                decoder.decompress_vec(&self.data)
                    .map_err(|e| AegisError::Storage(e.to_string()))?
            }
        };

        self.data = Bytes::from(decompressed);
        self.header.data_size = self.data.len() as u32;
        self.header.compression = CompressionType::None;
        self.update_checksum();

        Ok(())
    }

    /// Serialize the entire block to bytes.
    pub fn to_bytes(&self) -> Result<Bytes> {
        let header_bytes = self.header.to_bytes()?;
        let mut buf = BytesMut::with_capacity(header_bytes.len() + self.data.len() + 8);

        buf.put_u32_le(header_bytes.len() as u32);
        buf.put(header_bytes);
        buf.put_u32_le(self.data.len() as u32);
        buf.put(self.data.clone());

        Ok(buf.freeze())
    }

    /// Deserialize a block from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 8 {
            return Err(AegisError::Corruption("Block too small".to_string()));
        }

        let header_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + header_len + 4 {
            return Err(AegisError::Corruption("Block header truncated".to_string()));
        }

        let header = BlockHeader::from_bytes(&data[4..4 + header_len])?;

        let data_offset = 4 + header_len;
        let data_len = u32::from_le_bytes([
            data[data_offset],
            data[data_offset + 1],
            data[data_offset + 2],
            data[data_offset + 3],
        ]) as usize;

        if data.len() < data_offset + 4 + data_len {
            return Err(AegisError::Corruption("Block data truncated".to_string()));
        }

        let block_data = Bytes::copy_from_slice(&data[data_offset + 4..data_offset + 4 + data_len]);

        Ok(Self {
            header,
            data: block_data,
        })
    }

    /// Get the total size of the block in bytes.
    pub fn size(&self) -> usize {
        BLOCK_HEADER_SIZE + self.data.len()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_roundtrip() {
        let data = Bytes::from("Hello, Aegis!");
        let block = Block::new(BlockId(1), BlockType::TableData, data.clone());

        assert!(block.verify_checksum());

        let serialized = block.to_bytes().expect("to_bytes should succeed");
        let deserialized = Block::from_bytes(&serialized).expect("from_bytes should succeed");

        assert_eq!(deserialized.header.block_id, BlockId(1));
        assert_eq!(deserialized.data, data);
        assert!(deserialized.verify_checksum());
    }

    #[test]
    fn test_block_compression_lz4() {
        let data = Bytes::from("Hello, Aegis! ".repeat(100));
        let mut block = Block::new(BlockId(1), BlockType::TableData, data.clone());

        block.compress(CompressionType::Lz4).expect("LZ4 compression should succeed");
        assert!(block.header.data_size < block.header.uncompressed_size);

        block.decompress().expect("LZ4 decompression should succeed");
        assert_eq!(block.data, data);
    }

    #[test]
    fn test_block_compression_zstd() {
        let data = Bytes::from("Hello, Aegis! ".repeat(100));
        let mut block = Block::new(BlockId(1), BlockType::TableData, data.clone());

        block.compress(CompressionType::Zstd).expect("Zstd compression should succeed");
        assert!(block.header.data_size < block.header.uncompressed_size);

        block.decompress().expect("Zstd decompression should succeed");
        assert_eq!(block.data, data);
    }
}
