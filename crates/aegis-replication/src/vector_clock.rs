//! Aegis Vector Clocks
//!
//! Vector clocks for tracking causality and ordering in distributed systems.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::node::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::cmp::Ordering;

// =============================================================================
// Vector Clock
// =============================================================================

/// A vector clock for tracking causality between events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VectorClock {
    clocks: HashMap<String, u64>,
}

impl VectorClock {
    /// Create a new empty vector clock.
    pub fn new() -> Self {
        Self {
            clocks: HashMap::new(),
        }
    }

    /// Create a vector clock with initial node.
    pub fn with_node(node_id: &NodeId) -> Self {
        let mut clocks = HashMap::new();
        clocks.insert(node_id.as_str().to_string(), 0);
        Self { clocks }
    }

    /// Increment the clock for a node.
    pub fn increment(&mut self, node_id: &NodeId) {
        let key = node_id.as_str().to_string();
        *self.clocks.entry(key).or_insert(0) += 1;
    }

    /// Get the clock value for a node.
    pub fn get(&self, node_id: &NodeId) -> u64 {
        self.clocks
            .get(node_id.as_str())
            .copied()
            .unwrap_or(0)
    }

    /// Set the clock value for a node.
    pub fn set(&mut self, node_id: &NodeId, value: u64) {
        self.clocks.insert(node_id.as_str().to_string(), value);
    }

    /// Merge with another vector clock (take maximum of each component).
    pub fn merge(&mut self, other: &VectorClock) {
        for (node, &value) in &other.clocks {
            let current = self.clocks.entry(node.clone()).or_insert(0);
            *current = (*current).max(value);
        }
    }

    /// Create a merged clock without modifying self.
    pub fn merged(&self, other: &VectorClock) -> VectorClock {
        let mut result = self.clone();
        result.merge(other);
        result
    }

    /// Check if this clock happened before another.
    pub fn happened_before(&self, other: &VectorClock) -> bool {
        let mut dominated = false;

        // All our values must be <= other's values
        for (node, &value) in &self.clocks {
            let other_value = other.clocks.get(node).copied().unwrap_or(0);
            if value > other_value {
                return false;
            }
            if value < other_value {
                dominated = true;
            }
        }

        // Check for nodes in other that we don't have
        for (node, &value) in &other.clocks {
            if !self.clocks.contains_key(node) && value > 0 {
                dominated = true;
            }
        }

        dominated
    }

    /// Check if this clock happened after another.
    pub fn happened_after(&self, other: &VectorClock) -> bool {
        other.happened_before(self)
    }

    /// Check if two clocks are concurrent (neither happened before the other).
    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        !self.happened_before(other) && !self.happened_after(other)
    }

    /// Check if two clocks are identical.
    pub fn equals(&self, other: &VectorClock) -> bool {
        if self.clocks.len() != other.clocks.len() {
            return false;
        }

        for (node, &value) in &self.clocks {
            if other.clocks.get(node).copied().unwrap_or(0) != value {
                return false;
            }
        }

        true
    }

    /// Compare two vector clocks.
    pub fn compare(&self, other: &VectorClock) -> VectorClockOrdering {
        if self.equals(other) {
            VectorClockOrdering::Equal
        } else if self.happened_before(other) {
            VectorClockOrdering::Before
        } else if self.happened_after(other) {
            VectorClockOrdering::After
        } else {
            VectorClockOrdering::Concurrent
        }
    }

    /// Get the number of nodes tracked.
    pub fn node_count(&self) -> usize {
        self.clocks.len()
    }

    /// Get all nodes in this clock.
    pub fn nodes(&self) -> Vec<String> {
        self.clocks.keys().cloned().collect()
    }

    /// Get the maximum clock value across all nodes.
    pub fn max_value(&self) -> u64 {
        self.clocks.values().copied().max().unwrap_or(0)
    }

    /// Get the sum of all clock values.
    pub fn sum(&self) -> u64 {
        self.clocks.values().sum()
    }

    /// Check if the clock is empty (all zeros or no entries).
    pub fn is_empty(&self) -> bool {
        self.clocks.values().all(|&v| v == 0)
    }

    /// Reset all clock values to zero.
    pub fn reset(&mut self) {
        for value in self.clocks.values_mut() {
            *value = 0;
        }
    }

    /// Remove a node from the clock.
    pub fn remove(&mut self, node_id: &NodeId) {
        self.clocks.remove(node_id.as_str());
    }
}

impl PartialEq for VectorClock {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl Eq for VectorClock {}

impl PartialOrd for VectorClock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.compare(other) {
            VectorClockOrdering::Equal => Some(Ordering::Equal),
            VectorClockOrdering::Before => Some(Ordering::Less),
            VectorClockOrdering::After => Some(Ordering::Greater),
            VectorClockOrdering::Concurrent => None,
        }
    }
}

// =============================================================================
// Vector Clock Ordering
// =============================================================================

/// Result of comparing two vector clocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorClockOrdering {
    /// First clock happened before second.
    Before,
    /// First clock happened after second.
    After,
    /// Clocks are equal.
    Equal,
    /// Clocks are concurrent (incomparable).
    Concurrent,
}

// =============================================================================
// Versioned Value
// =============================================================================

/// A value with an associated vector clock version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedValue<T> {
    pub value: T,
    pub clock: VectorClock,
    pub timestamp: u64,
}

impl<T> VersionedValue<T> {
    /// Create a new versioned value.
    pub fn new(value: T, clock: VectorClock) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            value,
            clock,
            timestamp,
        }
    }

    /// Create with a specific timestamp.
    pub fn with_timestamp(value: T, clock: VectorClock, timestamp: u64) -> Self {
        Self {
            value,
            clock,
            timestamp,
        }
    }

    /// Check if this version happened before another.
    pub fn happened_before(&self, other: &VersionedValue<T>) -> bool {
        self.clock.happened_before(&other.clock)
    }

    /// Check if this version happened after another.
    pub fn happened_after(&self, other: &VersionedValue<T>) -> bool {
        self.clock.happened_after(&other.clock)
    }

    /// Check if this version is concurrent with another.
    pub fn is_concurrent(&self, other: &VersionedValue<T>) -> bool {
        self.clock.is_concurrent(&other.clock)
    }
}

// =============================================================================
// Hybrid Logical Clock
// =============================================================================

/// A hybrid logical clock combining physical time with logical counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridClock {
    physical: u64,
    logical: u32,
    node_id: String,
}

impl HybridClock {
    /// Create a new hybrid clock.
    pub fn new(node_id: impl Into<String>) -> Self {
        Self {
            physical: Self::now(),
            logical: 0,
            node_id: node_id.into(),
        }
    }

    /// Get current physical time in milliseconds.
    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Generate a new timestamp.
    pub fn tick(&mut self) -> HybridTimestamp {
        let now = Self::now();

        if now > self.physical {
            self.physical = now;
            self.logical = 0;
        } else {
            self.logical += 1;
        }

        HybridTimestamp {
            physical: self.physical,
            logical: self.logical,
            node_id: self.node_id.clone(),
        }
    }

    /// Update from a received timestamp.
    pub fn receive(&mut self, other: &HybridTimestamp) -> HybridTimestamp {
        let now = Self::now();

        if now > self.physical && now > other.physical {
            self.physical = now;
            self.logical = 0;
        } else if self.physical > other.physical {
            self.logical += 1;
        } else if other.physical > self.physical {
            self.physical = other.physical;
            self.logical = other.logical + 1;
        } else {
            // physical times are equal
            self.logical = self.logical.max(other.logical) + 1;
        }

        HybridTimestamp {
            physical: self.physical,
            logical: self.logical,
            node_id: self.node_id.clone(),
        }
    }

    /// Get current physical component.
    pub fn physical(&self) -> u64 {
        self.physical
    }

    /// Get current logical component.
    pub fn logical(&self) -> u32 {
        self.logical
    }
}

// =============================================================================
// Hybrid Timestamp
// =============================================================================

/// A timestamp from a hybrid logical clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridTimestamp {
    pub physical: u64,
    pub logical: u32,
    pub node_id: String,
}

impl HybridTimestamp {
    /// Create a new hybrid timestamp.
    pub fn new(physical: u64, logical: u32, node_id: impl Into<String>) -> Self {
        Self {
            physical,
            logical,
            node_id: node_id.into(),
        }
    }

    /// Compare with another timestamp.
    pub fn compare(&self, other: &HybridTimestamp) -> Ordering {
        match self.physical.cmp(&other.physical) {
            Ordering::Equal => match self.logical.cmp(&other.logical) {
                Ordering::Equal => self.node_id.cmp(&other.node_id),
                other => other,
            },
            other => other,
        }
    }

    /// Check if this timestamp is before another.
    pub fn is_before(&self, other: &HybridTimestamp) -> bool {
        self.compare(other) == Ordering::Less
    }

    /// Check if this timestamp is after another.
    pub fn is_after(&self, other: &HybridTimestamp) -> bool {
        self.compare(other) == Ordering::Greater
    }

    /// Convert to a sortable u128 value.
    pub fn to_sortable(&self) -> u128 {
        ((self.physical as u128) << 32) | (self.logical as u128)
    }
}

impl PartialOrd for HybridTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HybridTimestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

// =============================================================================
// Lamport Clock
// =============================================================================

/// A simple Lamport logical clock.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LamportClock {
    counter: u64,
}

impl LamportClock {
    /// Create a new Lamport clock.
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Create with initial value.
    pub fn with_value(value: u64) -> Self {
        Self { counter: value }
    }

    /// Increment and return the new timestamp.
    pub fn tick(&mut self) -> u64 {
        self.counter += 1;
        self.counter
    }

    /// Update from a received timestamp.
    pub fn receive(&mut self, received: u64) -> u64 {
        self.counter = self.counter.max(received) + 1;
        self.counter
    }

    /// Get current value.
    pub fn value(&self) -> u64 {
        self.counter
    }

    /// Merge with another clock.
    pub fn merge(&mut self, other: &LamportClock) {
        self.counter = self.counter.max(other.counter);
    }
}

// =============================================================================
// Dotted Version Vector
// =============================================================================

/// A dotted version vector for tracking object versions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DottedVersionVector {
    base: VectorClock,
    dots: HashMap<String, u64>,
}

impl DottedVersionVector {
    /// Create a new dotted version vector.
    pub fn new() -> Self {
        Self {
            base: VectorClock::new(),
            dots: HashMap::new(),
        }
    }

    /// Create a new event at a node.
    pub fn event(&mut self, node_id: &NodeId) -> (String, u64) {
        let key = node_id.as_str().to_string();
        let value = self.base.get(node_id) + 1;
        self.dots.insert(key.clone(), value);
        (key, value)
    }

    /// Sync the dot for a node into the base clock.
    pub fn sync(&mut self, node_id: &NodeId) {
        let key = node_id.as_str();
        if let Some(&dot) = self.dots.get(key) {
            let base_value = self.base.get(node_id);
            if dot == base_value + 1 {
                self.base.set(node_id, dot);
                self.dots.remove(key);
            }
        }
    }

    /// Sync all dots.
    pub fn sync_all(&mut self) {
        let nodes: Vec<_> = self.dots.keys().cloned().collect();
        for node in nodes {
            self.sync(&NodeId::new(&node));
        }
    }

    /// Merge with another dotted version vector.
    pub fn merge(&mut self, other: &DottedVersionVector) {
        self.base.merge(&other.base);

        for (node, &value) in &other.dots {
            let current = self.dots.entry(node.clone()).or_insert(0);
            *current = (*current).max(value);
        }
    }

    /// Check if this DVV dominates another.
    pub fn dominates(&self, other: &DottedVersionVector) -> bool {
        // Check all entries in other
        for (node, &value) in &other.base.clocks {
            let our_base = self.base.clocks.get(node).copied().unwrap_or(0);
            let our_dot = self.dots.get(node).copied().unwrap_or(0);
            if our_base.max(our_dot) < value {
                return false;
            }
        }

        for (node, &value) in &other.dots {
            let our_base = self.base.clocks.get(node).copied().unwrap_or(0);
            let our_dot = self.dots.get(node).copied().unwrap_or(0);
            if our_base.max(our_dot) < value {
                return false;
            }
        }

        true
    }

    /// Get the base vector clock.
    pub fn base(&self) -> &VectorClock {
        &self.base
    }

    /// Get the dots.
    pub fn dots(&self) -> &HashMap<String, u64> {
        &self.dots
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_clock_basic() {
        let mut clock = VectorClock::new();
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        clock.increment(&node_a);
        clock.increment(&node_a);
        clock.increment(&node_b);

        assert_eq!(clock.get(&node_a), 2);
        assert_eq!(clock.get(&node_b), 1);
        assert_eq!(clock.node_count(), 2);
    }

    #[test]
    fn test_vector_clock_happened_before() {
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        let mut clock1 = VectorClock::new();
        clock1.set(&node_a, 1);
        clock1.set(&node_b, 1);

        let mut clock2 = VectorClock::new();
        clock2.set(&node_a, 2);
        clock2.set(&node_b, 2);

        assert!(clock1.happened_before(&clock2));
        assert!(!clock2.happened_before(&clock1));
    }

    #[test]
    fn test_vector_clock_concurrent() {
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        let mut clock1 = VectorClock::new();
        clock1.set(&node_a, 2);
        clock1.set(&node_b, 1);

        let mut clock2 = VectorClock::new();
        clock2.set(&node_a, 1);
        clock2.set(&node_b, 2);

        assert!(clock1.is_concurrent(&clock2));
        assert!(!clock1.happened_before(&clock2));
        assert!(!clock2.happened_before(&clock1));
    }

    #[test]
    fn test_vector_clock_merge() {
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        let mut clock1 = VectorClock::new();
        clock1.set(&node_a, 2);
        clock1.set(&node_b, 1);

        let mut clock2 = VectorClock::new();
        clock2.set(&node_a, 1);
        clock2.set(&node_b, 3);

        clock1.merge(&clock2);

        assert_eq!(clock1.get(&node_a), 2);
        assert_eq!(clock1.get(&node_b), 3);
    }

    #[test]
    fn test_vector_clock_compare() {
        let node_a = NodeId::new("A");

        let mut clock1 = VectorClock::new();
        clock1.set(&node_a, 1);

        let mut clock2 = VectorClock::new();
        clock2.set(&node_a, 2);

        let clock3 = clock1.clone();

        assert_eq!(clock1.compare(&clock2), VectorClockOrdering::Before);
        assert_eq!(clock2.compare(&clock1), VectorClockOrdering::After);
        assert_eq!(clock1.compare(&clock3), VectorClockOrdering::Equal);
    }

    #[test]
    fn test_versioned_value() {
        let node_a = NodeId::new("A");

        let mut clock1 = VectorClock::new();
        clock1.set(&node_a, 1);

        let mut clock2 = VectorClock::new();
        clock2.set(&node_a, 2);

        let v1 = VersionedValue::new("value1", clock1);
        let v2 = VersionedValue::new("value2", clock2);

        assert!(v1.happened_before(&v2));
        assert!(!v1.is_concurrent(&v2));
    }

    #[test]
    fn test_hybrid_clock() {
        let mut clock = HybridClock::new("node1");

        let ts1 = clock.tick();
        let ts2 = clock.tick();

        assert!(ts1.is_before(&ts2));
        assert!(ts2.is_after(&ts1));
    }

    #[test]
    fn test_hybrid_clock_receive() {
        let mut clock1 = HybridClock::new("node1");
        let mut clock2 = HybridClock::new("node2");

        let ts1 = clock1.tick();
        let ts2 = clock2.receive(&ts1);

        assert!(ts1.is_before(&ts2));
    }

    #[test]
    fn test_hybrid_timestamp_ordering() {
        let ts1 = HybridTimestamp::new(100, 0, "A");
        let ts2 = HybridTimestamp::new(100, 1, "A");
        let ts3 = HybridTimestamp::new(101, 0, "A");

        assert!(ts1 < ts2);
        assert!(ts2 < ts3);
        assert!(ts1 < ts3);
    }

    #[test]
    fn test_lamport_clock() {
        let mut clock = LamportClock::new();

        assert_eq!(clock.tick(), 1);
        assert_eq!(clock.tick(), 2);

        let received = clock.receive(10);
        assert_eq!(received, 11);

        assert_eq!(clock.tick(), 12);
    }

    #[test]
    fn test_lamport_clock_merge() {
        let mut clock1 = LamportClock::with_value(5);
        let clock2 = LamportClock::with_value(10);

        clock1.merge(&clock2);
        assert_eq!(clock1.value(), 10);
    }

    #[test]
    fn test_dotted_version_vector() {
        let mut dvv = DottedVersionVector::new();
        let node_a = NodeId::new("A");

        let (node, version) = dvv.event(&node_a);
        assert_eq!(node, "A");
        assert_eq!(version, 1);

        dvv.sync(&node_a);
        assert_eq!(dvv.base().get(&node_a), 1);
    }

    #[test]
    fn test_dvv_merge() {
        let mut dvv1 = DottedVersionVector::new();
        let mut dvv2 = DottedVersionVector::new();

        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        dvv1.event(&node_a);
        dvv1.sync(&node_a);

        dvv2.event(&node_b);
        dvv2.sync(&node_b);

        dvv1.merge(&dvv2);

        assert_eq!(dvv1.base().get(&node_a), 1);
        assert_eq!(dvv1.base().get(&node_b), 1);
    }

    #[test]
    fn test_vector_clock_partial_ord() {
        let node_a = NodeId::new("A");

        let mut clock1 = VectorClock::new();
        clock1.set(&node_a, 1);

        let mut clock2 = VectorClock::new();
        clock2.set(&node_a, 2);

        assert!(clock1 < clock2);
        assert!(clock2 > clock1);
    }

    #[test]
    fn test_vector_clock_is_empty() {
        let clock1 = VectorClock::new();
        assert!(clock1.is_empty());

        let mut clock2 = VectorClock::new();
        clock2.increment(&NodeId::new("A"));
        assert!(!clock2.is_empty());
    }

    #[test]
    fn test_vector_clock_reset() {
        let mut clock = VectorClock::new();
        clock.increment(&NodeId::new("A"));
        clock.increment(&NodeId::new("B"));

        assert!(!clock.is_empty());

        clock.reset();
        assert!(clock.is_empty());
        assert_eq!(clock.node_count(), 2);
    }
}
