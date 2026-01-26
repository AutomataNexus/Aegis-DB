//! Aegis Arena - Arena-Based Memory Allocation
//!
//! Fast bump allocator for temporary allocations during query execution.
//! All allocations from an arena are freed together when the arena is reset,
//! eliminating per-allocation overhead.
//!
//! Key Features:
//! - O(1) allocation via pointer bumping
//! - Bulk deallocation on arena reset
//! - Configurable chunk sizes
//! - Thread-safe allocation with atomic operations
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use parking_lot::Mutex;
use std::alloc::{alloc, dealloc, Layout};
use std::ptr::NonNull;

// =============================================================================
// Constants
// =============================================================================

const DEFAULT_CHUNK_SIZE: usize = 64 * 1024; // 64 KB
const ALIGNMENT: usize = 8;

// =============================================================================
// Memory Chunk
// =============================================================================

struct Chunk {
    data: NonNull<u8>,
    size: usize,
    used: usize,
}

impl Chunk {
    fn new(size: usize) -> Option<Self> {
        let layout = Layout::from_size_align(size, ALIGNMENT).ok()?;
        let ptr = unsafe { alloc(layout) };
        let data = NonNull::new(ptr)?;

        Some(Self {
            data,
            size,
            used: 0,
        })
    }

    fn allocate(&mut self, size: usize, align: usize) -> Option<NonNull<u8>> {
        let align_mask = align - 1;
        let aligned_used = (self.used + align_mask) & !align_mask;
        let new_used = aligned_used + size;

        if new_used > self.size {
            return None;
        }

        let ptr = unsafe { self.data.as_ptr().add(aligned_used) };
        self.used = new_used;

        NonNull::new(ptr)
    }

    #[allow(dead_code)]
    fn remaining(&self) -> usize {
        self.size - self.used
    }

    fn reset(&mut self) {
        self.used = 0;
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        if let Ok(layout) = Layout::from_size_align(self.size, ALIGNMENT) {
            unsafe {
                dealloc(self.data.as_ptr(), layout);
            }
        }
    }
}

// =============================================================================
// Memory Arena
// =============================================================================

/// Arena allocator for fast temporary allocations.
pub struct MemoryArena {
    chunks: Mutex<Vec<Chunk>>,
    chunk_size: usize,
    total_allocated: Mutex<usize>,
}

impl MemoryArena {
    /// Create a new arena with default chunk size.
    pub fn new() -> Self {
        Self::with_chunk_size(DEFAULT_CHUNK_SIZE)
    }

    /// Create a new arena with specified chunk size.
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            chunks: Mutex::new(Vec::new()),
            chunk_size,
            total_allocated: Mutex::new(0),
        }
    }

    /// Allocate memory from the arena.
    pub fn allocate(&self, size: usize) -> Option<NonNull<u8>> {
        self.allocate_aligned(size, ALIGNMENT)
    }

    /// Allocate aligned memory from the arena.
    pub fn allocate_aligned(&self, size: usize, align: usize) -> Option<NonNull<u8>> {
        let mut chunks = self.chunks.lock();

        if let Some(chunk) = chunks.last_mut() {
            if let Some(ptr) = chunk.allocate(size, align) {
                return Some(ptr);
            }
        }

        let chunk_size = std::cmp::max(self.chunk_size, size + align);
        let mut new_chunk = Chunk::new(chunk_size)?;
        let ptr = new_chunk.allocate(size, align);

        *self.total_allocated.lock() += chunk_size;
        chunks.push(new_chunk);

        ptr
    }

    /// Allocate and initialize memory with a value.
    #[allow(clippy::mut_from_ref)] // Arena uses interior mutability (Mutex)
    pub fn allocate_value<T>(&self, value: T) -> Option<&mut T> {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        let ptr = self.allocate_aligned(size, align)?;

        unsafe {
            let typed_ptr = ptr.as_ptr() as *mut T;
            std::ptr::write(typed_ptr, value);
            Some(&mut *typed_ptr)
        }
    }

    /// Allocate a slice from the arena.
    #[allow(clippy::mut_from_ref)] // Arena uses interior mutability (Mutex)
    pub fn allocate_slice<T: Copy>(&self, len: usize) -> Option<&mut [T]> {
        let size = std::mem::size_of::<T>() * len;
        let align = std::mem::align_of::<T>();
        let ptr = self.allocate_aligned(size, align)?;

        unsafe {
            let typed_ptr = ptr.as_ptr() as *mut T;
            Some(std::slice::from_raw_parts_mut(typed_ptr, len))
        }
    }

    /// Reset the arena, freeing all allocations.
    pub fn reset(&self) {
        let mut chunks = self.chunks.lock();
        for chunk in chunks.iter_mut() {
            chunk.reset();
        }
    }

    /// Clear the arena completely, deallocating all chunks.
    pub fn clear(&self) {
        let mut chunks = self.chunks.lock();
        chunks.clear();
        *self.total_allocated.lock() = 0;
    }

    /// Get the total memory allocated by this arena.
    pub fn total_allocated(&self) -> usize {
        *self.total_allocated.lock()
    }

    /// Get the total memory used within allocated chunks.
    pub fn total_used(&self) -> usize {
        self.chunks.lock().iter().map(|c| c.used).sum()
    }
}

impl Default for MemoryArena {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_allocation() {
        let arena = MemoryArena::new();

        let ptr1 = arena.allocate(100).expect("First allocation should succeed");
        let ptr2 = arena.allocate(200).expect("Second allocation should succeed");

        // NonNull guarantees non-null, so just check they're different
        assert_ne!(ptr1.as_ptr(), ptr2.as_ptr());
    }

    #[test]
    fn test_arena_value_allocation() {
        let arena = MemoryArena::new();

        let value = arena.allocate_value(42u64).expect("Value allocation should succeed");
        assert_eq!(*value, 42);

        *value = 100;
        assert_eq!(*value, 100);
    }

    #[test]
    fn test_arena_slice_allocation() {
        let arena = MemoryArena::new();

        let slice = arena.allocate_slice::<u32>(10).expect("Slice allocation should succeed");
        assert_eq!(slice.len(), 10);

        for (i, v) in slice.iter_mut().enumerate() {
            *v = i as u32;
        }

        for (i, v) in slice.iter().enumerate() {
            assert_eq!(*v, i as u32);
        }
    }

    #[test]
    fn test_arena_reset() {
        let arena = MemoryArena::new();

        arena.allocate(1000).expect("Allocation should succeed");
        let used_before = arena.total_used();
        assert!(used_before > 0);

        arena.reset();
        let used_after = arena.total_used();
        assert_eq!(used_after, 0);
    }

    #[test]
    fn test_arena_large_allocation() {
        let arena = MemoryArena::with_chunk_size(1024);

        // Just verify the allocation succeeds - NonNull guarantees non-null
        let _ptr = arena.allocate(2048).expect("Large allocation should succeed");
    }
}
