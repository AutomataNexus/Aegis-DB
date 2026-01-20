//! Aegis Buffer - Buffer Pool Management
//!
//! LRU-based buffer pool for caching pages in memory. Provides efficient
//! page access with automatic eviction of least-recently-used pages when
//! the pool reaches capacity.
//!
//! Key Features:
//! - LRU eviction policy for cache management
//! - Pin counting to protect active pages from eviction
//! - Dirty page tracking for write-back optimization
//! - Thread-safe concurrent access
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::page::{Page, PageType, PAGE_SIZE};
use aegis_common::{PageId, Result, AegisError};
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

// =============================================================================
// Buffer Pool Configuration
// =============================================================================

/// Configuration for the buffer pool.
#[derive(Debug, Clone)]
pub struct BufferPoolConfig {
    pub pool_size: usize,
    pub prefetch_size: usize,
}

impl Default for BufferPoolConfig {
    fn default() -> Self {
        Self {
            pool_size: 1024,
            prefetch_size: 8,
        }
    }
}

// =============================================================================
// Buffer Frame
// =============================================================================

/// A frame in the buffer pool holding a page.
struct BufferFrame {
    page: RwLock<Option<Page>>,
    page_id: RwLock<Option<PageId>>,
}

impl BufferFrame {
    fn new() -> Self {
        Self {
            page: RwLock::new(None),
            page_id: RwLock::new(None),
        }
    }

    fn is_occupied(&self) -> bool {
        self.page_id.read().is_some()
    }

    fn get_page_id(&self) -> Option<PageId> {
        *self.page_id.read()
    }
}

// =============================================================================
// Buffer Pool
// =============================================================================

/// LRU buffer pool for page caching.
pub struct BufferPool {
    frames: Vec<Arc<BufferFrame>>,
    page_table: RwLock<HashMap<PageId, usize>>,
    free_list: Mutex<VecDeque<usize>>,
    lru_list: Mutex<VecDeque<usize>>,
    config: BufferPoolConfig,
}

impl BufferPool {
    /// Create a new buffer pool with the given configuration.
    pub fn new(config: BufferPoolConfig) -> Self {
        let mut frames = Vec::with_capacity(config.pool_size);
        let mut free_list = VecDeque::with_capacity(config.pool_size);

        for i in 0..config.pool_size {
            frames.push(Arc::new(BufferFrame::new()));
            free_list.push_back(i);
        }

        Self {
            frames,
            page_table: RwLock::new(HashMap::new()),
            free_list: Mutex::new(free_list),
            lru_list: Mutex::new(VecDeque::new()),
            config,
        }
    }

    /// Create a buffer pool with default configuration.
    pub fn with_capacity(num_pages: usize) -> Self {
        Self::new(BufferPoolConfig {
            pool_size: num_pages,
            ..Default::default()
        })
    }

    /// Fetch a page from the pool, loading it if necessary.
    pub fn fetch_page(&self, page_id: PageId) -> Result<PageHandle> {
        if let Some(&frame_id) = self.page_table.read().get(&page_id) {
            let frame = &self.frames[frame_id];
            if let Some(ref page) = *frame.page.read() {
                page.pin();
                self.update_lru(frame_id);
                return Ok(PageHandle {
                    frame: Arc::clone(frame),
                    page_id,
                });
            }
        }

        let frame_id = self.get_free_frame()?;
        let frame = &self.frames[frame_id];

        let page = Page::new(page_id, PageType::Data);
        page.pin();

        *frame.page.write() = Some(page);
        *frame.page_id.write() = Some(page_id);

        self.page_table.write().insert(page_id, frame_id);
        self.update_lru(frame_id);

        Ok(PageHandle {
            frame: Arc::clone(frame),
            page_id,
        })
    }

    /// Create a new page in the pool.
    pub fn new_page(&self, page_id: PageId, page_type: PageType) -> Result<PageHandle> {
        if self.page_table.read().contains_key(&page_id) {
            return Err(AegisError::Storage(format!(
                "Page {} already exists",
                page_id.0
            )));
        }

        let frame_id = self.get_free_frame()?;
        let frame = &self.frames[frame_id];

        let page = Page::new(page_id, page_type);
        page.pin();

        *frame.page.write() = Some(page);
        *frame.page_id.write() = Some(page_id);

        self.page_table.write().insert(page_id, frame_id);
        self.update_lru(frame_id);

        Ok(PageHandle {
            frame: Arc::clone(frame),
            page_id,
        })
    }

    /// Flush a specific page to storage.
    pub fn flush_page(&self, page_id: PageId) -> Result<Option<Page>> {
        if let Some(&frame_id) = self.page_table.read().get(&page_id) {
            let frame = &self.frames[frame_id];
            let page_guard = frame.page.read();

            if let Some(ref page) = *page_guard {
                if page.is_dirty() {
                    let cloned = Page::from_bytes(&page.to_bytes())?;
                    page.clear_dirty();
                    return Ok(Some(cloned));
                }
            }
        }
        Ok(None)
    }

    /// Unpin a page, allowing it to be evicted.
    pub fn unpin_page(&self, page_id: PageId) {
        if let Some(&frame_id) = self.page_table.read().get(&page_id) {
            let frame = &self.frames[frame_id];
            if let Some(ref page) = *frame.page.read() {
                page.unpin();
            }
        }
    }

    /// Get buffer pool statistics.
    pub fn stats(&self) -> BufferPoolStats {
        let page_table = self.page_table.read();
        let free_count = self.free_list.lock().len();

        let mut dirty_count = 0;
        let mut pinned_count = 0;

        for &frame_id in page_table.values() {
            let frame = &self.frames[frame_id];
            if let Some(ref page) = *frame.page.read() {
                if page.is_dirty() {
                    dirty_count += 1;
                }
                if page.is_pinned() {
                    pinned_count += 1;
                }
            }
        }

        BufferPoolStats {
            total_frames: self.config.pool_size,
            used_frames: page_table.len(),
            free_frames: free_count,
            dirty_pages: dirty_count,
            pinned_pages: pinned_count,
        }
    }

    fn get_free_frame(&self) -> Result<usize> {
        if let Some(frame_id) = self.free_list.lock().pop_front() {
            return Ok(frame_id);
        }

        self.evict_page()
    }

    fn evict_page(&self) -> Result<usize> {
        let mut lru_list = self.lru_list.lock();

        for _ in 0..lru_list.len() {
            if let Some(frame_id) = lru_list.pop_front() {
                let frame = &self.frames[frame_id];

                if let Some(ref page) = *frame.page.read() {
                    if page.is_pinned() {
                        lru_list.push_back(frame_id);
                        continue;
                    }
                }

                if let Some(page_id) = frame.get_page_id() {
                    self.page_table.write().remove(&page_id);
                }

                *frame.page.write() = None;
                *frame.page_id.write() = None;

                return Ok(frame_id);
            }
        }

        Err(AegisError::ResourceExhausted(
            "No evictable pages in buffer pool".to_string(),
        ))
    }

    fn update_lru(&self, frame_id: usize) {
        let mut lru_list = self.lru_list.lock();
        lru_list.retain(|&id| id != frame_id);
        lru_list.push_back(frame_id);
    }
}

// =============================================================================
// Page Handle
// =============================================================================

/// RAII handle for accessing a page in the buffer pool.
pub struct PageHandle {
    frame: Arc<BufferFrame>,
    page_id: PageId,
}

impl PageHandle {
    /// Get a read reference to the page.
    pub fn read(&self) -> impl std::ops::Deref<Target = Page> + '_ {
        parking_lot::RwLockReadGuard::map(self.frame.page.read(), |opt| {
            opt.as_ref().expect("Page should be present")
        })
    }

    /// Get a write reference to the page.
    pub fn write(&self) -> impl std::ops::DerefMut<Target = Page> + '_ {
        parking_lot::RwLockWriteGuard::map(self.frame.page.write(), |opt| {
            opt.as_mut().expect("Page should be present")
        })
    }

    /// Get the page ID.
    pub fn page_id(&self) -> PageId {
        self.page_id
    }
}

impl Drop for PageHandle {
    fn drop(&mut self) {
        if let Some(ref page) = *self.frame.page.read() {
            page.unpin();
        }
    }
}

// =============================================================================
// Statistics
// =============================================================================

/// Buffer pool statistics.
#[derive(Debug, Clone)]
pub struct BufferPoolStats {
    pub total_frames: usize,
    pub used_frames: usize,
    pub free_frames: usize,
    pub dirty_pages: usize,
    pub pinned_pages: usize,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_new_page() {
        let pool = BufferPool::with_capacity(10);
        let handle = pool.new_page(PageId(1), PageType::Data).unwrap();

        assert_eq!(handle.page_id(), PageId(1));
    }

    #[test]
    fn test_buffer_pool_fetch_page() {
        let pool = BufferPool::with_capacity(10);

        pool.new_page(PageId(1), PageType::Data).unwrap();

        let handle = pool.fetch_page(PageId(1)).unwrap();
        assert_eq!(handle.page_id(), PageId(1));
    }

    #[test]
    fn test_buffer_pool_eviction() {
        let pool = BufferPool::with_capacity(3);

        let _h1 = pool.new_page(PageId(1), PageType::Data).unwrap();
        let _h2 = pool.new_page(PageId(2), PageType::Data).unwrap();
        let _h3 = pool.new_page(PageId(3), PageType::Data).unwrap();

        drop(_h1);
        drop(_h2);
        drop(_h3);

        let _h4 = pool.new_page(PageId(4), PageType::Data).unwrap();

        let stats = pool.stats();
        assert_eq!(stats.used_frames, 3);
    }

    #[test]
    fn test_buffer_pool_stats() {
        let pool = BufferPool::with_capacity(10);

        let stats = pool.stats();
        assert_eq!(stats.total_frames, 10);
        assert_eq!(stats.used_frames, 0);
        assert_eq!(stats.free_frames, 10);

        let _h1 = pool.new_page(PageId(1), PageType::Data).unwrap();
        let _h2 = pool.new_page(PageId(2), PageType::Data).unwrap();

        let stats = pool.stats();
        assert_eq!(stats.used_frames, 2);
    }
}
