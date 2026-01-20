//! Aegis Page - Buffer Pool Page Management
//!
//! Fixed-size pages for the buffer pool cache. Pages provide a higher-level
//! abstraction over blocks for row storage, indexing, and metadata.
//!
//! Key Features:
//! - Fixed 8KB page size for consistent I/O
//! - Dirty tracking for write-back caching
//! - Pin counting to prevent eviction during use
//! - Slot-based row storage format
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use aegis_common::{PageId, Result, AegisError};
use bytes::{Bytes, BytesMut, BufMut, Buf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

// =============================================================================
// Constants
// =============================================================================

pub const PAGE_SIZE: usize = 8192;
pub const PAGE_HEADER_SIZE: usize = 24;
pub const MAX_TUPLE_SIZE: usize = PAGE_SIZE - PAGE_HEADER_SIZE - 8;

// =============================================================================
// Page Header
// =============================================================================

/// Page header containing metadata and slot directory info.
#[derive(Debug, Clone)]
pub struct PageHeader {
    pub page_id: PageId,
    pub page_type: PageType,
    pub num_slots: u16,
    pub free_space_start: u16,
    pub free_space_end: u16,
    pub flags: u16,
}

impl PageHeader {
    pub fn new(page_id: PageId, page_type: PageType) -> Self {
        Self {
            page_id,
            page_type,
            num_slots: 0,
            free_space_start: PAGE_HEADER_SIZE as u16,
            free_space_end: PAGE_SIZE as u16,
            flags: 0,
        }
    }

    pub fn free_space(&self) -> usize {
        (self.free_space_end - self.free_space_start) as usize
    }
}

// =============================================================================
// Page Type
// =============================================================================

/// Classification of page contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PageType {
    Data = 1,
    Index = 2,
    Overflow = 3,
    FreeList = 4,
    Metadata = 5,
}

impl From<u8> for PageType {
    fn from(value: u8) -> Self {
        match value {
            1 => PageType::Data,
            2 => PageType::Index,
            3 => PageType::Overflow,
            4 => PageType::FreeList,
            _ => PageType::Metadata,
        }
    }
}

// =============================================================================
// Slot
// =============================================================================

/// Slot directory entry pointing to tuple data.
#[derive(Debug, Clone, Copy)]
pub struct Slot {
    pub offset: u16,
    pub length: u16,
}

impl Slot {
    pub const SIZE: usize = 4;

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}

// =============================================================================
// Page
// =============================================================================

/// A fixed-size page in the buffer pool.
pub struct Page {
    pub header: PageHeader,
    data: BytesMut,
    pin_count: AtomicU32,
    is_dirty: AtomicBool,
}

impl Page {
    /// Create a new empty page.
    pub fn new(page_id: PageId, page_type: PageType) -> Self {
        let mut data = BytesMut::zeroed(PAGE_SIZE);
        let header = PageHeader::new(page_id, page_type);

        Self::write_header(&mut data, &header);

        Self {
            header,
            data,
            pin_count: AtomicU32::new(0),
            is_dirty: AtomicBool::new(false),
        }
    }

    /// Create a page from raw bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() != PAGE_SIZE {
            return Err(AegisError::Corruption(format!(
                "Invalid page size: {} (expected {})",
                data.len(),
                PAGE_SIZE
            )));
        }

        let mut buf = &data[..];
        let page_id = PageId(buf.get_u64_le());
        let page_type = PageType::from(buf.get_u8());
        let _padding = buf.get_u8();
        let num_slots = buf.get_u16_le();
        let free_space_start = buf.get_u16_le();
        let free_space_end = buf.get_u16_le();
        let flags = buf.get_u16_le();

        let header = PageHeader {
            page_id,
            page_type,
            num_slots,
            free_space_start,
            free_space_end,
            flags,
        };

        Ok(Self {
            header,
            data: BytesMut::from(data),
            pin_count: AtomicU32::new(0),
            is_dirty: AtomicBool::new(false),
        })
    }

    fn write_header(data: &mut BytesMut, header: &PageHeader) {
        let mut buf = &mut data[..PAGE_HEADER_SIZE];
        buf.put_u64_le(header.page_id.0);
        buf.put_u8(header.page_type as u8);
        buf.put_u8(0); // padding
        buf.put_u16_le(header.num_slots);
        buf.put_u16_le(header.free_space_start);
        buf.put_u16_le(header.free_space_end);
        buf.put_u16_le(header.flags);
    }

    /// Serialize page to bytes.
    pub fn to_bytes(&self) -> Bytes {
        self.data.clone().freeze()
    }

    /// Insert a tuple into the page.
    pub fn insert_tuple(&mut self, tuple: &[u8]) -> Result<u16> {
        let tuple_len = tuple.len();
        let required_space = tuple_len + Slot::SIZE;

        if required_space > self.header.free_space() {
            return Err(AegisError::Storage("Page full".to_string()));
        }

        if tuple_len > MAX_TUPLE_SIZE {
            return Err(AegisError::Storage("Tuple too large".to_string()));
        }

        let slot_offset = PAGE_HEADER_SIZE + (self.header.num_slots as usize * Slot::SIZE);
        let tuple_offset = self.header.free_space_end as usize - tuple_len;

        self.data[tuple_offset..tuple_offset + tuple_len].copy_from_slice(tuple);

        let slot = Slot {
            offset: tuple_offset as u16,
            length: tuple_len as u16,
        };
        self.write_slot(self.header.num_slots, slot);

        let slot_id = self.header.num_slots;
        self.header.num_slots += 1;
        self.header.free_space_start = (slot_offset + Slot::SIZE) as u16;
        self.header.free_space_end = tuple_offset as u16;

        Self::write_header(&mut self.data, &self.header);
        self.mark_dirty();

        Ok(slot_id)
    }

    /// Read a tuple from the page.
    pub fn read_tuple(&self, slot_id: u16) -> Result<&[u8]> {
        if slot_id >= self.header.num_slots {
            return Err(AegisError::Storage("Invalid slot ID".to_string()));
        }

        let slot = self.read_slot(slot_id);
        if slot.is_empty() {
            return Err(AegisError::Storage("Slot is empty".to_string()));
        }

        let offset = slot.offset as usize;
        let length = slot.length as usize;

        Ok(&self.data[offset..offset + length])
    }

    /// Delete a tuple from the page (marks slot as empty).
    pub fn delete_tuple(&mut self, slot_id: u16) -> Result<()> {
        if slot_id >= self.header.num_slots {
            return Err(AegisError::Storage("Invalid slot ID".to_string()));
        }

        let slot = Slot {
            offset: 0,
            length: 0,
        };
        self.write_slot(slot_id, slot);
        self.mark_dirty();

        Ok(())
    }

    fn read_slot(&self, slot_id: u16) -> Slot {
        let offset = PAGE_HEADER_SIZE + (slot_id as usize * Slot::SIZE);
        let buf = &self.data[offset..];

        Slot {
            offset: u16::from_le_bytes([buf[0], buf[1]]),
            length: u16::from_le_bytes([buf[2], buf[3]]),
        }
    }

    fn write_slot(&mut self, slot_id: u16, slot: Slot) {
        let offset = PAGE_HEADER_SIZE + (slot_id as usize * Slot::SIZE);
        self.data[offset..offset + 2].copy_from_slice(&slot.offset.to_le_bytes());
        self.data[offset + 2..offset + 4].copy_from_slice(&slot.length.to_le_bytes());
    }

    /// Pin the page to prevent eviction.
    pub fn pin(&self) {
        self.pin_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Unpin the page.
    pub fn unpin(&self) {
        self.pin_count.fetch_sub(1, Ordering::SeqCst);
    }

    /// Get the current pin count.
    pub fn pin_count(&self) -> u32 {
        self.pin_count.load(Ordering::SeqCst)
    }

    /// Check if the page is pinned.
    pub fn is_pinned(&self) -> bool {
        self.pin_count() > 0
    }

    /// Mark the page as dirty.
    pub fn mark_dirty(&self) {
        self.is_dirty.store(true, Ordering::SeqCst);
    }

    /// Check if the page is dirty.
    pub fn is_dirty(&self) -> bool {
        self.is_dirty.load(Ordering::SeqCst)
    }

    /// Clear the dirty flag.
    pub fn clear_dirty(&self) {
        self.is_dirty.store(false, Ordering::SeqCst);
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_insert_read() {
        let mut page = Page::new(PageId(1), PageType::Data);
        let tuple = b"Hello, Aegis!";

        let slot_id = page.insert_tuple(tuple).unwrap();
        let read_tuple = page.read_tuple(slot_id).unwrap();

        assert_eq!(read_tuple, tuple);
    }

    #[test]
    fn test_page_multiple_tuples() {
        let mut page = Page::new(PageId(1), PageType::Data);

        let slot1 = page.insert_tuple(b"First tuple").unwrap();
        let slot2 = page.insert_tuple(b"Second tuple").unwrap();
        let slot3 = page.insert_tuple(b"Third tuple").unwrap();

        assert_eq!(page.read_tuple(slot1).unwrap(), b"First tuple");
        assert_eq!(page.read_tuple(slot2).unwrap(), b"Second tuple");
        assert_eq!(page.read_tuple(slot3).unwrap(), b"Third tuple");
    }

    #[test]
    fn test_page_delete() {
        let mut page = Page::new(PageId(1), PageType::Data);
        let slot_id = page.insert_tuple(b"Delete me").unwrap();

        page.delete_tuple(slot_id).unwrap();
        assert!(page.read_tuple(slot_id).is_err());
    }

    #[test]
    fn test_page_serialization() {
        let mut page = Page::new(PageId(42), PageType::Data);
        page.insert_tuple(b"Test data").unwrap();

        let bytes = page.to_bytes();
        let restored = Page::from_bytes(&bytes).unwrap();

        assert_eq!(restored.header.page_id, PageId(42));
        assert_eq!(restored.header.num_slots, 1);
    }
}
