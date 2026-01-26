//! Aegis CRDTs (Conflict-free Replicated Data Types)
//!
//! Data structures that can be replicated across nodes and merged without conflicts.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::node::NodeId;
use crate::vector_clock::VectorClock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// =============================================================================
// CRDT Trait
// =============================================================================

/// Trait for conflict-free replicated data types.
pub trait CRDT: Clone {
    /// Merge with another instance.
    fn merge(&mut self, other: &Self);

    /// Create a merged instance without modifying self.
    fn merged(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.merge(other);
        result
    }
}

// =============================================================================
// G-Counter (Grow-only Counter)
// =============================================================================

/// A grow-only counter that can only be incremented.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GCounter {
    counts: HashMap<String, u64>,
}

impl GCounter {
    /// Create a new G-Counter.
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// Increment the counter for a node.
    pub fn increment(&mut self, node_id: &NodeId) {
        let key = node_id.as_str().to_string();
        *self.counts.entry(key).or_insert(0) += 1;
    }

    /// Increment by a specific amount.
    pub fn increment_by(&mut self, node_id: &NodeId, amount: u64) {
        let key = node_id.as_str().to_string();
        *self.counts.entry(key).or_insert(0) += amount;
    }

    /// Get the total value of the counter.
    pub fn value(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Get the value for a specific node.
    pub fn node_value(&self, node_id: &NodeId) -> u64 {
        self.counts.get(node_id.as_str()).copied().unwrap_or(0)
    }
}

impl CRDT for GCounter {
    fn merge(&mut self, other: &Self) {
        for (node, &value) in &other.counts {
            let current = self.counts.entry(node.clone()).or_insert(0);
            *current = (*current).max(value);
        }
    }
}

// =============================================================================
// PN-Counter (Positive-Negative Counter)
// =============================================================================

/// A counter that supports both increment and decrement.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PNCounter {
    positive: GCounter,
    negative: GCounter,
}

impl PNCounter {
    /// Create a new PN-Counter.
    pub fn new() -> Self {
        Self {
            positive: GCounter::new(),
            negative: GCounter::new(),
        }
    }

    /// Increment the counter.
    pub fn increment(&mut self, node_id: &NodeId) {
        self.positive.increment(node_id);
    }

    /// Increment by a specific amount.
    pub fn increment_by(&mut self, node_id: &NodeId, amount: u64) {
        self.positive.increment_by(node_id, amount);
    }

    /// Decrement the counter.
    pub fn decrement(&mut self, node_id: &NodeId) {
        self.negative.increment(node_id);
    }

    /// Decrement by a specific amount.
    pub fn decrement_by(&mut self, node_id: &NodeId, amount: u64) {
        self.negative.increment_by(node_id, amount);
    }

    /// Get the current value (can be negative).
    pub fn value(&self) -> i64 {
        self.positive.value() as i64 - self.negative.value() as i64
    }
}

impl CRDT for PNCounter {
    fn merge(&mut self, other: &Self) {
        self.positive.merge(&other.positive);
        self.negative.merge(&other.negative);
    }
}

// =============================================================================
// G-Set (Grow-only Set)
// =============================================================================

/// A set that only supports add operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GSet<T: Clone + Eq + std::hash::Hash + Serialize> {
    elements: HashSet<T>,
}

impl<T: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>> GSet<T> {
    /// Create a new G-Set.
    pub fn new() -> Self {
        Self {
            elements: HashSet::new(),
        }
    }

    /// Add an element to the set.
    pub fn add(&mut self, element: T) {
        self.elements.insert(element);
    }

    /// Check if the set contains an element.
    pub fn contains(&self, element: &T) -> bool {
        self.elements.contains(element)
    }

    /// Get the number of elements.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Iterate over elements.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.elements.iter()
    }

    /// Convert to a Vec.
    pub fn to_vec(&self) -> Vec<T> {
        self.elements.iter().cloned().collect()
    }
}

impl<T: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>> CRDT for GSet<T> {
    fn merge(&mut self, other: &Self) {
        for element in &other.elements {
            self.elements.insert(element.clone());
        }
    }
}

// =============================================================================
// 2P-Set (Two-Phase Set)
// =============================================================================

/// A set that supports add and remove (elements can only be removed once).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TwoPSet<T: Clone + Eq + std::hash::Hash + Serialize> {
    added: HashSet<T>,
    removed: HashSet<T>,
}

impl<T: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>> TwoPSet<T> {
    /// Create a new 2P-Set.
    pub fn new() -> Self {
        Self {
            added: HashSet::new(),
            removed: HashSet::new(),
        }
    }

    /// Add an element to the set.
    pub fn add(&mut self, element: T) {
        if !self.removed.contains(&element) {
            self.added.insert(element);
        }
    }

    /// Remove an element from the set.
    pub fn remove(&mut self, element: T) {
        if self.added.contains(&element) {
            self.removed.insert(element);
        }
    }

    /// Check if the set contains an element.
    pub fn contains(&self, element: &T) -> bool {
        self.added.contains(element) && !self.removed.contains(element)
    }

    /// Get all active elements.
    pub fn elements(&self) -> HashSet<T> {
        self.added
            .difference(&self.removed)
            .cloned()
            .collect()
    }

    /// Get the number of active elements.
    pub fn len(&self) -> usize {
        self.elements().len()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.elements().is_empty()
    }
}

impl<T: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>> CRDT for TwoPSet<T> {
    fn merge(&mut self, other: &Self) {
        for element in &other.added {
            self.added.insert(element.clone());
        }
        for element in &other.removed {
            self.removed.insert(element.clone());
        }
    }
}

// =============================================================================
// LWW-Register (Last-Writer-Wins Register)
// =============================================================================

/// A register where the most recent write wins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWRegister<T: Clone + Serialize> {
    value: Option<T>,
    timestamp: u64,
    node_id: String,
}

impl<T: Clone + Serialize + for<'de> Deserialize<'de>> LWWRegister<T> {
    /// Create a new empty register.
    pub fn new() -> Self {
        Self {
            value: None,
            timestamp: 0,
            node_id: String::new(),
        }
    }

    /// Create a register with an initial value.
    pub fn with_value(value: T, node_id: &NodeId) -> Self {
        Self {
            value: Some(value),
            timestamp: Self::now(),
            node_id: node_id.as_str().to_string(),
        }
    }

    /// Get current timestamp.
    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    /// Set the register value.
    pub fn set(&mut self, value: T, node_id: &NodeId) {
        let ts = Self::now();
        if ts > self.timestamp || (ts == self.timestamp && node_id.as_str() > &self.node_id) {
            self.value = Some(value);
            self.timestamp = ts;
            self.node_id = node_id.as_str().to_string();
        }
    }

    /// Set with a specific timestamp.
    pub fn set_with_timestamp(&mut self, value: T, timestamp: u64, node_id: &NodeId) {
        if timestamp > self.timestamp
            || (timestamp == self.timestamp && node_id.as_str() > &self.node_id)
        {
            self.value = Some(value);
            self.timestamp = timestamp;
            self.node_id = node_id.as_str().to_string();
        }
    }

    /// Get the register value.
    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Get the timestamp.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Check if the register is set.
    pub fn is_set(&self) -> bool {
        self.value.is_some()
    }
}

impl<T: Clone + Serialize + for<'de> Deserialize<'de>> Default for LWWRegister<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Serialize + for<'de> Deserialize<'de>> CRDT for LWWRegister<T> {
    fn merge(&mut self, other: &Self) {
        if other.timestamp > self.timestamp
            || (other.timestamp == self.timestamp && other.node_id > self.node_id)
        {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
            self.node_id = other.node_id.clone();
        }
    }
}

// =============================================================================
// MV-Register (Multi-Value Register)
// =============================================================================

/// A register that keeps all concurrent values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MVRegister<T: Clone + Eq + std::hash::Hash + Serialize> {
    values: HashMap<T, VectorClock>,
}

impl<T: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>> MVRegister<T> {
    /// Create a new empty register.
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Set the register value.
    pub fn set(&mut self, value: T, clock: VectorClock) {
        // Remove values dominated by the new clock
        self.values.retain(|_, v| !v.happened_before(&clock));

        // Add new value if not dominated
        let dominated = self.values.values().any(|v| clock.happened_before(v));
        if !dominated {
            self.values.insert(value, clock);
        }
    }

    /// Get all current values.
    pub fn get(&self) -> Vec<&T> {
        self.values.keys().collect()
    }

    /// Get values with their clocks.
    pub fn get_with_clocks(&self) -> Vec<(&T, &VectorClock)> {
        self.values.iter().collect()
    }

    /// Check if there are concurrent values (conflict).
    pub fn has_conflict(&self) -> bool {
        self.values.len() > 1
    }

    /// Get the number of concurrent values.
    pub fn value_count(&self) -> usize {
        self.values.len()
    }
}

impl<T: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>> Default
    for MVRegister<T>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>> CRDT
    for MVRegister<T>
{
    fn merge(&mut self, other: &Self) {
        for (value, clock) in &other.values {
            self.set(value.clone(), clock.clone());
        }
    }
}

// =============================================================================
// OR-Set (Observed-Remove Set)
// =============================================================================

/// Unique identifier for OR-Set elements.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UniqueTag {
    node_id: String,
    counter: u64,
}

impl UniqueTag {
    /// Create a new unique tag.
    pub fn new(node_id: &NodeId, counter: u64) -> Self {
        Self {
            node_id: node_id.as_str().to_string(),
            counter,
        }
    }
}

/// An observed-remove set that supports add and remove without conflicts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ORSet<T: Clone + Eq + std::hash::Hash + Serialize> {
    elements: HashMap<T, HashSet<UniqueTag>>,
    counters: HashMap<String, u64>,
}

impl<T: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>> ORSet<T> {
    /// Create a new OR-Set.
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
            counters: HashMap::new(),
        }
    }

    /// Add an element to the set.
    pub fn add(&mut self, element: T, node_id: &NodeId) {
        let counter = self
            .counters
            .entry(node_id.as_str().to_string())
            .or_insert(0);
        *counter += 1;

        let tag = UniqueTag::new(node_id, *counter);
        self.elements
            .entry(element)
            .or_default()
            .insert(tag);
    }

    /// Remove an element from the set.
    pub fn remove(&mut self, element: &T) {
        self.elements.remove(element);
    }

    /// Check if the set contains an element.
    pub fn contains(&self, element: &T) -> bool {
        self.elements
            .get(element)
            .map(|tags| !tags.is_empty())
            .unwrap_or(false)
    }

    /// Get all elements.
    pub fn elements(&self) -> Vec<&T> {
        self.elements
            .iter()
            .filter(|(_, tags)| !tags.is_empty())
            .map(|(elem, _)| elem)
            .collect()
    }

    /// Get the number of elements.
    pub fn len(&self) -> usize {
        self.elements
            .iter()
            .filter(|(_, tags)| !tags.is_empty())
            .count()
    }

    /// Check if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>> CRDT for ORSet<T> {
    fn merge(&mut self, other: &Self) {
        for (element, tags) in &other.elements {
            let our_tags = self.elements.entry(element.clone()).or_default();
            for tag in tags {
                our_tags.insert(tag.clone());
            }
        }

        for (node, &counter) in &other.counters {
            let our_counter = self.counters.entry(node.clone()).or_insert(0);
            *our_counter = (*our_counter).max(counter);
        }
    }
}

// =============================================================================
// LWW-Map (Last-Writer-Wins Map)
// =============================================================================

/// A map where the most recent write for each key wins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWMap<K: Clone + Eq + std::hash::Hash + Serialize, V: Clone + Serialize> {
    entries: HashMap<K, LWWRegister<V>>,
}

impl<
        K: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>,
        V: Clone + Serialize + for<'de> Deserialize<'de>,
    > LWWMap<K, V>
{
    /// Create a new LWW-Map.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Set a key-value pair.
    pub fn set(&mut self, key: K, value: V, node_id: &NodeId) {
        self.entries
            .entry(key)
            .or_default()
            .set(value, node_id);
    }

    /// Get a value by key.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key).and_then(|r| r.get())
    }

    /// Remove a key (by setting to a tombstone timestamp).
    pub fn remove(&mut self, key: &K) {
        self.entries.remove(key);
    }

    /// Get all keys.
    pub fn keys(&self) -> Vec<&K> {
        self.entries
            .iter()
            .filter(|(_, v)| v.is_set())
            .map(|(k, _)| k)
            .collect()
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|(_, v)| v.is_set()).count()
    }

    /// Check if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<
        K: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>,
        V: Clone + Serialize + for<'de> Deserialize<'de>,
    > Default for LWWMap<K, V>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<
        K: Clone + Eq + std::hash::Hash + Serialize + for<'de> Deserialize<'de>,
        V: Clone + Serialize + for<'de> Deserialize<'de>,
    > CRDT for LWWMap<K, V>
{
    fn merge(&mut self, other: &Self) {
        for (key, register) in &other.entries {
            self.entries
                .entry(key.clone())
                .or_default()
                .merge(register);
        }
    }
}

// =============================================================================
// Delta-CRDT Support
// =============================================================================

/// Trait for delta-state CRDTs.
pub trait DeltaCRDT: CRDT {
    /// The type of delta operations.
    type Delta: Clone + Serialize;

    /// Apply a delta operation.
    fn apply_delta(&mut self, delta: &Self::Delta);

    /// Generate a delta for a mutation.
    fn generate_delta(&self) -> Option<Self::Delta>;
}

/// Delta for G-Counter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCounterDelta {
    pub node_id: String,
    pub value: u64,
}

impl DeltaCRDT for GCounter {
    type Delta = GCounterDelta;

    fn apply_delta(&mut self, delta: &Self::Delta) {
        let current = self.counts.entry(delta.node_id.clone()).or_insert(0);
        *current = (*current).max(delta.value);
    }

    fn generate_delta(&self) -> Option<Self::Delta> {
        // Would typically track what changed since last delta
        None
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcounter_basic() {
        let mut counter = GCounter::new();
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        counter.increment(&node_a);
        counter.increment(&node_a);
        counter.increment(&node_b);

        assert_eq!(counter.value(), 3);
        assert_eq!(counter.node_value(&node_a), 2);
        assert_eq!(counter.node_value(&node_b), 1);
    }

    #[test]
    fn test_gcounter_merge() {
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        let mut counter1 = GCounter::new();
        counter1.increment(&node_a);
        counter1.increment(&node_a);

        let mut counter2 = GCounter::new();
        counter2.increment(&node_b);
        counter2.increment(&node_b);
        counter2.increment(&node_b);

        counter1.merge(&counter2);
        assert_eq!(counter1.value(), 5);
    }

    #[test]
    fn test_pncounter() {
        let mut counter = PNCounter::new();
        let node = NodeId::new("A");

        counter.increment(&node);
        counter.increment(&node);
        counter.increment(&node);
        counter.decrement(&node);

        assert_eq!(counter.value(), 2);
    }

    #[test]
    fn test_pncounter_negative() {
        let mut counter = PNCounter::new();
        let node = NodeId::new("A");

        counter.decrement(&node);
        counter.decrement(&node);

        assert_eq!(counter.value(), -2);
    }

    #[test]
    fn test_gset() {
        let mut set: GSet<String> = GSet::new();

        set.add("apple".to_string());
        set.add("banana".to_string());
        set.add("apple".to_string()); // duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains(&"apple".to_string()));
        assert!(!set.contains(&"cherry".to_string()));
    }

    #[test]
    fn test_gset_merge() {
        let mut set1: GSet<String> = GSet::new();
        set1.add("apple".to_string());

        let mut set2: GSet<String> = GSet::new();
        set2.add("banana".to_string());

        set1.merge(&set2);
        assert_eq!(set1.len(), 2);
    }

    #[test]
    fn test_twopset() {
        let mut set: TwoPSet<String> = TwoPSet::new();

        set.add("apple".to_string());
        set.add("banana".to_string());

        assert!(set.contains(&"apple".to_string()));

        set.remove("apple".to_string());
        assert!(!set.contains(&"apple".to_string()));

        // Can't re-add removed element
        set.add("apple".to_string());
        assert!(!set.contains(&"apple".to_string()));
    }

    #[test]
    fn test_lww_register() {
        let mut reg: LWWRegister<String> = LWWRegister::new();
        let node = NodeId::new("A");

        reg.set("value1".to_string(), &node);
        assert_eq!(reg.get(), Some(&"value1".to_string()));

        reg.set("value2".to_string(), &node);
        assert_eq!(reg.get(), Some(&"value2".to_string()));
    }

    #[test]
    fn test_lww_register_merge() {
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        let mut reg1: LWWRegister<String> = LWWRegister::new();
        reg1.set_with_timestamp("older".to_string(), 100, &node_a);

        let mut reg2: LWWRegister<String> = LWWRegister::new();
        reg2.set_with_timestamp("newer".to_string(), 200, &node_b);

        reg1.merge(&reg2);
        assert_eq!(reg1.get(), Some(&"newer".to_string()));
    }

    #[test]
    fn test_mv_register() {
        let mut reg: MVRegister<String> = MVRegister::new();
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        let mut clock1 = VectorClock::new();
        clock1.set(&node_a, 1);

        let mut clock2 = VectorClock::new();
        clock2.set(&node_b, 1);

        // Two concurrent writes
        reg.set("value_a".to_string(), clock1);
        reg.set("value_b".to_string(), clock2);

        assert!(reg.has_conflict());
        assert_eq!(reg.value_count(), 2);
    }

    #[test]
    fn test_orset() {
        let mut set: ORSet<String> = ORSet::new();
        let node = NodeId::new("A");

        set.add("apple".to_string(), &node);
        set.add("banana".to_string(), &node);

        assert!(set.contains(&"apple".to_string()));
        assert_eq!(set.len(), 2);

        set.remove(&"apple".to_string());
        assert!(!set.contains(&"apple".to_string()));

        // Can re-add after remove in OR-Set
        set.add("apple".to_string(), &node);
        assert!(set.contains(&"apple".to_string()));
    }

    #[test]
    fn test_orset_merge() {
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        let mut set1: ORSet<String> = ORSet::new();
        set1.add("apple".to_string(), &node_a);

        let mut set2: ORSet<String> = ORSet::new();
        set2.add("banana".to_string(), &node_b);

        set1.merge(&set2);

        assert!(set1.contains(&"apple".to_string()));
        assert!(set1.contains(&"banana".to_string()));
    }

    #[test]
    fn test_orset_concurrent_add_remove() {
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        let mut set1: ORSet<String> = ORSet::new();
        set1.add("apple".to_string(), &node_a);

        // Clone before remove
        let mut set2 = set1.clone();

        // Node A removes
        set1.remove(&"apple".to_string());

        // Node B adds concurrently
        set2.add("apple".to_string(), &node_b);

        // Merge: add wins because B's add has a different tag
        set1.merge(&set2);
        assert!(set1.contains(&"apple".to_string()));
    }

    #[test]
    fn test_lww_map() {
        let mut map: LWWMap<String, i32> = LWWMap::new();
        let node = NodeId::new("A");

        map.set("key1".to_string(), 100, &node);
        map.set("key2".to_string(), 200, &node);

        assert_eq!(map.get(&"key1".to_string()), Some(&100));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_lww_map_merge() {
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        let mut map1: LWWMap<String, i32> = LWWMap::new();
        map1.set("key1".to_string(), 100, &node_a);

        let mut map2: LWWMap<String, i32> = LWWMap::new();
        map2.set("key2".to_string(), 200, &node_b);

        map1.merge(&map2);

        assert_eq!(map1.get(&"key1".to_string()), Some(&100));
        assert_eq!(map1.get(&"key2".to_string()), Some(&200));
    }

    #[test]
    fn test_gcounter_delta() {
        let mut counter = GCounter::new();
        let node = NodeId::new("A");

        counter.increment(&node);

        let delta = GCounterDelta {
            node_id: "B".to_string(),
            value: 5,
        };

        counter.apply_delta(&delta);
        assert_eq!(counter.value(), 6);
    }

    #[test]
    fn test_crdt_merged() {
        let node_a = NodeId::new("A");
        let node_b = NodeId::new("B");

        let mut counter1 = GCounter::new();
        counter1.increment(&node_a);

        let mut counter2 = GCounter::new();
        counter2.increment(&node_b);

        let merged = counter1.merged(&counter2);

        // Original unchanged
        assert_eq!(counter1.value(), 1);
        // Merged has both
        assert_eq!(merged.value(), 2);
    }
}
