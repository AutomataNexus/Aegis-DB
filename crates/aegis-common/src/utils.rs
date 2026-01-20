//! Aegis Utils - Utility Functions
//!
//! Common utility functions used across the Aegis database platform.
//! Provides hashing, checksum validation, and size formatting utilities.
//!
//! Key Features:
//! - Fast hashing using xxHash3 for internal structures
//! - CRC32 checksums for data integrity verification
//! - Human-readable size formatting
//! - Consistent key generation for distributed operations
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use xxhash_rust::xxh3::{xxh3_64, xxh3_128};

// =============================================================================
// Hashing Functions
// =============================================================================

/// Compute a 64-bit hash of the given bytes using xxHash3.
#[inline]
pub fn hash64(data: &[u8]) -> u64 {
    xxh3_64(data)
}

/// Compute a 128-bit hash of the given bytes using xxHash3.
#[inline]
pub fn hash128(data: &[u8]) -> u128 {
    xxh3_128(data)
}

/// Compute a hash suitable for consistent hashing ring placement.
#[inline]
pub fn ring_hash(key: &[u8]) -> u64 {
    hash64(key)
}

// =============================================================================
// Checksum Functions
// =============================================================================

/// Compute CRC32 checksum for data integrity verification.
#[inline]
pub fn crc32(data: &[u8]) -> u32 {
    crc32fast::hash(data)
}

/// Verify data against expected CRC32 checksum.
#[inline]
pub fn verify_crc32(data: &[u8], expected: u32) -> bool {
    crc32(data) == expected
}

// =============================================================================
// Size Formatting
// =============================================================================

const SIZE_UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];

/// Format a byte size as a human-readable string.
pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < SIZE_UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, SIZE_UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, SIZE_UNITS[unit_index])
    }
}

/// Parse a human-readable size string to bytes.
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();

    let (num_str, unit) = if s.ends_with("PB") {
        (&s[..s.len() - 2], 1024_u64.pow(5))
    } else if s.ends_with("TB") {
        (&s[..s.len() - 2], 1024_u64.pow(4))
    } else if s.ends_with("GB") {
        (&s[..s.len() - 2], 1024_u64.pow(3))
    } else if s.ends_with("MB") {
        (&s[..s.len() - 2], 1024_u64.pow(2))
    } else if s.ends_with("KB") {
        (&s[..s.len() - 2], 1024_u64)
    } else if s.ends_with('B') {
        (&s[..s.len() - 1], 1_u64)
    } else {
        (s.as_str(), 1_u64)
    };

    num_str.trim().parse::<f64>().ok().map(|n| (n * unit as f64) as u64)
}

// =============================================================================
// Alignment Utilities
// =============================================================================

/// Align a value up to the nearest multiple of alignment.
#[inline]
pub const fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

/// Align a value down to the nearest multiple of alignment.
#[inline]
pub const fn align_down(value: usize, alignment: usize) -> usize {
    value & !(alignment - 1)
}

/// Check if a value is aligned to the given alignment.
#[inline]
pub const fn is_aligned(value: usize, alignment: usize) -> bool {
    value & (alignment - 1) == 0
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1024"), Some(1024));
        assert_eq!(parse_size("1KB"), Some(1024));
        assert_eq!(parse_size("1 MB"), Some(1024 * 1024));
        assert_eq!(parse_size("1.5GB"), Some((1.5 * 1024.0 * 1024.0 * 1024.0) as u64));
    }

    #[test]
    fn test_alignment() {
        assert_eq!(align_up(0, 8), 0);
        assert_eq!(align_up(1, 8), 8);
        assert_eq!(align_up(8, 8), 8);
        assert_eq!(align_up(9, 8), 16);

        assert_eq!(align_down(0, 8), 0);
        assert_eq!(align_down(7, 8), 0);
        assert_eq!(align_down(8, 8), 8);
        assert_eq!(align_down(15, 8), 8);

        assert!(is_aligned(0, 8));
        assert!(is_aligned(8, 8));
        assert!(!is_aligned(7, 8));
    }

    #[test]
    fn test_crc32() {
        let data = b"hello world";
        let checksum = crc32(data);
        assert!(verify_crc32(data, checksum));
        assert!(!verify_crc32(b"hello worlD", checksum));
    }
}
