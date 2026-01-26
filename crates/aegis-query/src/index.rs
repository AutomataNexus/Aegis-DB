//! Query Engine Indexes
//!
//! B-tree and hash index implementations for query optimization.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::RwLock;

use aegis_common::Value;

// =============================================================================
// Index Types
// =============================================================================

/// The type of index to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[derive(Default)]
pub enum IndexType {
    /// B-tree index for range queries and ordered scans.
    #[default]
    BTree,
    /// Hash index for equality lookups (faster for exact matches).
    Hash,
}


// =============================================================================
// Index Key
// =============================================================================

/// A key in an index, supporting composite keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndexKey {
    pub values: Vec<IndexValue>,
}

/// A single value in an index key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IndexValue {
    Null,
    Bool(bool),
    Int(i64),
    String(String),
    /// For floats, we use ordered representation for BTree compatibility
    Float(OrderedFloat),
}

/// Wrapper for f64 that implements Ord for BTree storage.
#[derive(Debug, Clone, Copy)]
pub struct OrderedFloat(pub f64);

impl PartialEq for OrderedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for OrderedFloat {}

impl std::hash::Hash for OrderedFloat {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl IndexKey {
    pub fn new(values: Vec<IndexValue>) -> Self {
        Self { values }
    }

    pub fn single(value: IndexValue) -> Self {
        Self { values: vec![value] }
    }

    /// Convert a Value to an IndexValue.
    pub fn from_value(value: &Value) -> IndexValue {
        match value {
            Value::Null => IndexValue::Null,
            Value::Boolean(b) => IndexValue::Bool(*b),
            Value::Integer(i) => IndexValue::Int(*i),
            Value::Float(f) => IndexValue::Float(OrderedFloat(*f)),
            Value::String(s) => IndexValue::String(s.clone()),
            Value::Timestamp(t) => IndexValue::Int(t.timestamp()),
            Value::Bytes(b) => IndexValue::String(format!("{:?}", b)),
            Value::Array(arr) => IndexValue::String(format!("{:?}", arr)),
            Value::Object(obj) => IndexValue::String(format!("{:?}", obj)),
        }
    }

    /// Create an IndexKey from multiple Values.
    pub fn from_values(values: &[Value]) -> Self {
        Self {
            values: values.iter().map(Self::from_value).collect(),
        }
    }
}

impl PartialOrd for IndexKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IndexKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.values.cmp(&other.values)
    }
}

// =============================================================================
// Row ID
// =============================================================================

/// A row identifier in an index.
pub type RowId = usize;

// =============================================================================
// B-Tree Index
// =============================================================================

/// A B-tree index for range queries and ordered scans.
pub struct BTreeIndex {
    /// Index name.
    pub name: String,
    /// Table this index belongs to.
    pub table: String,
    /// Column names in the index (supports composite keys).
    pub columns: Vec<String>,
    /// Whether this is a unique index.
    pub unique: bool,
    /// The actual B-tree storing key -> row IDs.
    tree: RwLock<BTreeMap<IndexKey, HashSet<RowId>>>,
    /// Number of entries in the index.
    entry_count: RwLock<usize>,
}

impl BTreeIndex {
    /// Create a new B-tree index.
    pub fn new(name: String, table: String, columns: Vec<String>, unique: bool) -> Self {
        Self {
            name,
            table,
            columns,
            unique,
            tree: RwLock::new(BTreeMap::new()),
            entry_count: RwLock::new(0),
        }
    }

    /// Insert a key-rowid pair into the index.
    pub fn insert(&self, key: IndexKey, row_id: RowId) -> Result<(), IndexError> {
        let mut tree = self.tree.write().expect("BTreeIndex tree lock poisoned");

        if self.unique {
            if let Some(existing) = tree.get(&key) {
                if !existing.is_empty() {
                    return Err(IndexError::DuplicateKey(format!(
                        "Duplicate key in unique index '{}'",
                        self.name
                    )));
                }
            }
        }

        let entry = tree.entry(key).or_default();
        if entry.insert(row_id) {
            *self.entry_count.write().expect("BTreeIndex entry_count lock poisoned") += 1;
        }
        Ok(())
    }

    /// Remove a key-rowid pair from the index.
    pub fn remove(&self, key: &IndexKey, row_id: RowId) -> bool {
        let mut tree = self.tree.write().expect("BTreeIndex tree lock poisoned");
        if let Some(row_ids) = tree.get_mut(key) {
            if row_ids.remove(&row_id) {
                *self.entry_count.write().expect("BTreeIndex entry_count lock poisoned") -= 1;
                if row_ids.is_empty() {
                    tree.remove(key);
                }
                return true;
            }
        }
        false
    }

    /// Look up an exact key.
    pub fn get(&self, key: &IndexKey) -> Vec<RowId> {
        let tree = self.tree.read().expect("BTreeIndex tree lock poisoned");
        tree.get(key)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Range scan from start to end (inclusive/exclusive based on flags).
    pub fn range(
        &self,
        start: Option<&IndexKey>,
        end: Option<&IndexKey>,
        start_inclusive: bool,
        end_inclusive: bool,
    ) -> Vec<RowId> {
        let tree = self.tree.read().expect("BTreeIndex tree lock poisoned");
        let mut result = Vec::new();

        use std::ops::Bound;

        let start_bound = match start {
            Some(k) if start_inclusive => Bound::Included(k.clone()),
            Some(k) => Bound::Excluded(k.clone()),
            None => Bound::Unbounded,
        };

        let end_bound = match end {
            Some(k) if end_inclusive => Bound::Included(k.clone()),
            Some(k) => Bound::Excluded(k.clone()),
            None => Bound::Unbounded,
        };

        for (_, row_ids) in tree.range((start_bound, end_bound)) {
            result.extend(row_ids.iter().copied());
        }

        result
    }

    /// Get all row IDs in index order.
    pub fn scan_all(&self) -> Vec<RowId> {
        let tree = self.tree.read().expect("BTreeIndex tree lock poisoned");
        tree.values().flat_map(|ids| ids.iter().copied()).collect()
    }

    /// Get the number of entries in the index.
    pub fn len(&self) -> usize {
        *self.entry_count.read().expect("BTreeIndex entry_count lock poisoned")
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all entries from the index.
    pub fn clear(&self) {
        let mut tree = self.tree.write().expect("BTreeIndex tree lock poisoned");
        tree.clear();
        *self.entry_count.write().expect("BTreeIndex entry_count lock poisoned") = 0;
    }
}

// =============================================================================
// Hash Index
// =============================================================================

/// A hash index for fast equality lookups.
pub struct HashIndex {
    /// Index name.
    pub name: String,
    /// Table this index belongs to.
    pub table: String,
    /// Column names in the index.
    pub columns: Vec<String>,
    /// Whether this is a unique index.
    pub unique: bool,
    /// The hash map storing key -> row IDs.
    map: RwLock<HashMap<IndexKey, HashSet<RowId>>>,
    /// Number of entries in the index.
    entry_count: RwLock<usize>,
}

impl HashIndex {
    /// Create a new hash index.
    pub fn new(name: String, table: String, columns: Vec<String>, unique: bool) -> Self {
        Self {
            name,
            table,
            columns,
            unique,
            map: RwLock::new(HashMap::new()),
            entry_count: RwLock::new(0),
        }
    }

    /// Insert a key-rowid pair into the index.
    pub fn insert(&self, key: IndexKey, row_id: RowId) -> Result<(), IndexError> {
        let mut map = self.map.write().expect("HashIndex map lock poisoned");

        if self.unique {
            if let Some(existing) = map.get(&key) {
                if !existing.is_empty() {
                    return Err(IndexError::DuplicateKey(format!(
                        "Duplicate key in unique index '{}'",
                        self.name
                    )));
                }
            }
        }

        let entry = map.entry(key).or_default();
        if entry.insert(row_id) {
            *self.entry_count.write().expect("HashIndex entry_count lock poisoned") += 1;
        }
        Ok(())
    }

    /// Remove a key-rowid pair from the index.
    pub fn remove(&self, key: &IndexKey, row_id: RowId) -> bool {
        let mut map = self.map.write().expect("HashIndex map lock poisoned");
        if let Some(row_ids) = map.get_mut(key) {
            if row_ids.remove(&row_id) {
                *self.entry_count.write().expect("HashIndex entry_count lock poisoned") -= 1;
                if row_ids.is_empty() {
                    map.remove(key);
                }
                return true;
            }
        }
        false
    }

    /// Look up an exact key.
    pub fn get(&self, key: &IndexKey) -> Vec<RowId> {
        let map = self.map.read().expect("HashIndex map lock poisoned");
        map.get(key)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get the number of entries in the index.
    pub fn len(&self) -> usize {
        *self.entry_count.read().expect("HashIndex entry_count lock poisoned")
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all entries from the index.
    pub fn clear(&self) {
        let mut map = self.map.write().expect("HashIndex map lock poisoned");
        map.clear();
        *self.entry_count.write().expect("HashIndex entry_count lock poisoned") = 0;
    }
}

// =============================================================================
// Unified Index Trait
// =============================================================================

/// A trait for index operations.
pub trait Index: Send + Sync {
    /// Get the index name.
    fn name(&self) -> &str;

    /// Get the table name.
    fn table(&self) -> &str;

    /// Get the column names.
    fn columns(&self) -> &[String];

    /// Check if this is a unique index.
    fn is_unique(&self) -> bool;

    /// Get the index type.
    fn index_type(&self) -> IndexType;

    /// Insert a key-rowid pair.
    fn insert(&self, key: IndexKey, row_id: RowId) -> Result<(), IndexError>;

    /// Remove a key-rowid pair.
    fn remove(&self, key: &IndexKey, row_id: RowId) -> bool;

    /// Look up an exact key.
    fn get(&self, key: &IndexKey) -> Vec<RowId>;

    /// Get the number of entries.
    fn len(&self) -> usize;

    /// Check if empty.
    fn is_empty(&self) -> bool;

    /// Clear the index.
    fn clear(&self);
}

impl Index for BTreeIndex {
    fn name(&self) -> &str { &self.name }
    fn table(&self) -> &str { &self.table }
    fn columns(&self) -> &[String] { &self.columns }
    fn is_unique(&self) -> bool { self.unique }
    fn index_type(&self) -> IndexType { IndexType::BTree }
    fn insert(&self, key: IndexKey, row_id: RowId) -> Result<(), IndexError> { self.insert(key, row_id) }
    fn remove(&self, key: &IndexKey, row_id: RowId) -> bool { self.remove(key, row_id) }
    fn get(&self, key: &IndexKey) -> Vec<RowId> { self.get(key) }
    fn len(&self) -> usize { self.len() }
    fn is_empty(&self) -> bool { self.is_empty() }
    fn clear(&self) { self.clear() }
}

impl Index for HashIndex {
    fn name(&self) -> &str { &self.name }
    fn table(&self) -> &str { &self.table }
    fn columns(&self) -> &[String] { &self.columns }
    fn is_unique(&self) -> bool { self.unique }
    fn index_type(&self) -> IndexType { IndexType::Hash }
    fn insert(&self, key: IndexKey, row_id: RowId) -> Result<(), IndexError> { self.insert(key, row_id) }
    fn remove(&self, key: &IndexKey, row_id: RowId) -> bool { self.remove(key, row_id) }
    fn get(&self, key: &IndexKey) -> Vec<RowId> { self.get(key) }
    fn len(&self) -> usize { self.len() }
    fn is_empty(&self) -> bool { self.is_empty() }
    fn clear(&self) { self.clear() }
}

// =============================================================================
// Index Error
// =============================================================================

/// Errors that can occur during index operations.
#[derive(Debug, Clone)]
pub enum IndexError {
    DuplicateKey(String),
    IndexNotFound(String),
    InvalidKey(String),
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexError::DuplicateKey(msg) => write!(f, "Duplicate key: {}", msg),
            IndexError::IndexNotFound(msg) => write!(f, "Index not found: {}", msg),
            IndexError::InvalidKey(msg) => write!(f, "Invalid key: {}", msg),
        }
    }
}

impl std::error::Error for IndexError {}

// =============================================================================
// Index Manager
// =============================================================================

/// Manages all indexes for a table.
pub struct TableIndexManager {
    /// Table name.
    table: String,
    /// B-tree indexes by name.
    btree_indexes: RwLock<HashMap<String, BTreeIndex>>,
    /// Hash indexes by name.
    hash_indexes: RwLock<HashMap<String, HashIndex>>,
}

impl TableIndexManager {
    /// Create a new index manager for a table.
    pub fn new(table: String) -> Self {
        Self {
            table,
            btree_indexes: RwLock::new(HashMap::new()),
            hash_indexes: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new index.
    pub fn create_index(
        &self,
        name: String,
        columns: Vec<String>,
        unique: bool,
        index_type: IndexType,
    ) -> Result<(), IndexError> {
        match index_type {
            IndexType::BTree => {
                let mut indexes = self.btree_indexes.write().expect("TableIndexManager btree_indexes lock poisoned");
                if indexes.contains_key(&name) {
                    return Err(IndexError::DuplicateKey(format!("Index '{}' already exists", name)));
                }
                indexes.insert(
                    name.clone(),
                    BTreeIndex::new(name, self.table.clone(), columns, unique),
                );
            }
            IndexType::Hash => {
                let mut indexes = self.hash_indexes.write().expect("TableIndexManager hash_indexes lock poisoned");
                if indexes.contains_key(&name) {
                    return Err(IndexError::DuplicateKey(format!("Index '{}' already exists", name)));
                }
                indexes.insert(
                    name.clone(),
                    HashIndex::new(name, self.table.clone(), columns, unique),
                );
            }
        }
        Ok(())
    }

    /// Drop an index.
    pub fn drop_index(&self, name: &str) -> Result<(), IndexError> {
        let mut btree = self.btree_indexes.write().expect("TableIndexManager btree_indexes lock poisoned");
        if btree.remove(name).is_some() {
            return Ok(());
        }

        let mut hash = self.hash_indexes.write().expect("TableIndexManager hash_indexes lock poisoned");
        if hash.remove(name).is_some() {
            return Ok(());
        }

        Err(IndexError::IndexNotFound(name.to_string()))
    }

    /// Insert a row into all indexes.
    pub fn insert_row(
        &self,
        row_id: RowId,
        column_values: &HashMap<String, Value>,
    ) -> Result<(), IndexError> {
        // Insert into B-tree indexes
        let btree = self.btree_indexes.read().expect("TableIndexManager btree_indexes lock poisoned");
        for index in btree.values() {
            let key = self.build_key(&index.columns, column_values);
            index.insert(key, row_id)?;
        }

        // Insert into hash indexes
        let hash = self.hash_indexes.read().expect("TableIndexManager hash_indexes lock poisoned");
        for index in hash.values() {
            let key = self.build_key(&index.columns, column_values);
            index.insert(key, row_id)?;
        }

        Ok(())
    }

    /// Remove a row from all indexes.
    pub fn remove_row(&self, row_id: RowId, column_values: &HashMap<String, Value>) {
        // Remove from B-tree indexes
        let btree = self.btree_indexes.read().expect("TableIndexManager btree_indexes lock poisoned");
        for index in btree.values() {
            let key = self.build_key(&index.columns, column_values);
            index.remove(&key, row_id);
        }

        // Remove from hash indexes
        let hash = self.hash_indexes.read().expect("TableIndexManager hash_indexes lock poisoned");
        for index in hash.values() {
            let key = self.build_key(&index.columns, column_values);
            index.remove(&key, row_id);
        }
    }

    /// Update a row in all indexes (remove old, insert new).
    pub fn update_row(
        &self,
        row_id: RowId,
        old_values: &HashMap<String, Value>,
        new_values: &HashMap<String, Value>,
    ) -> Result<(), IndexError> {
        self.remove_row(row_id, old_values);
        self.insert_row(row_id, new_values)
    }

    /// Look up rows by index for equality.
    pub fn lookup_eq(&self, index_name: &str, key: &IndexKey) -> Option<Vec<RowId>> {
        // Check B-tree indexes
        let btree = self.btree_indexes.read().expect("TableIndexManager btree_indexes lock poisoned");
        if let Some(index) = btree.get(index_name) {
            return Some(index.get(key));
        }

        // Check hash indexes
        let hash = self.hash_indexes.read().expect("TableIndexManager hash_indexes lock poisoned");
        if let Some(index) = hash.get(index_name) {
            return Some(index.get(key));
        }

        None
    }

    /// Range lookup (only works for B-tree indexes).
    pub fn lookup_range(
        &self,
        index_name: &str,
        start: Option<&IndexKey>,
        end: Option<&IndexKey>,
        start_inclusive: bool,
        end_inclusive: bool,
    ) -> Option<Vec<RowId>> {
        let btree = self.btree_indexes.read().expect("TableIndexManager btree_indexes lock poisoned");
        btree.get(index_name).map(|index| {
            index.range(start, end, start_inclusive, end_inclusive)
        })
    }

    /// Find the best index for a set of columns.
    pub fn find_index_for_columns(&self, columns: &[String]) -> Option<(String, IndexType)> {
        // First try to find an exact match in hash indexes (faster for equality)
        let hash = self.hash_indexes.read().expect("TableIndexManager hash_indexes lock poisoned");
        for (name, index) in hash.iter() {
            if index.columns == columns {
                return Some((name.clone(), IndexType::Hash));
            }
        }

        // Then try B-tree indexes (can use prefix of columns)
        let btree = self.btree_indexes.read().expect("TableIndexManager btree_indexes lock poisoned");
        for (name, index) in btree.iter() {
            if columns.len() <= index.columns.len()
                && index.columns[..columns.len()] == columns[..]
            {
                return Some((name.clone(), IndexType::BTree));
            }
        }

        None
    }

    /// Get all index names.
    pub fn index_names(&self) -> Vec<String> {
        let btree = self.btree_indexes.read().expect("TableIndexManager btree_indexes lock poisoned");
        let hash = self.hash_indexes.read().expect("TableIndexManager hash_indexes lock poisoned");
        btree.keys().chain(hash.keys()).cloned().collect()
    }

    /// Get index info by name.
    pub fn get_index_info(&self, name: &str) -> Option<(Vec<String>, bool, IndexType)> {
        let btree = self.btree_indexes.read().expect("TableIndexManager btree_indexes lock poisoned");
        if let Some(index) = btree.get(name) {
            return Some((index.columns.clone(), index.unique, IndexType::BTree));
        }

        let hash = self.hash_indexes.read().expect("TableIndexManager hash_indexes lock poisoned");
        if let Some(index) = hash.get(name) {
            return Some((index.columns.clone(), index.unique, IndexType::Hash));
        }

        None
    }

    /// Insert a row into a specific index by name.
    pub fn insert_into_index(
        &self,
        index_name: &str,
        key: IndexKey,
        row_id: RowId,
    ) -> Result<(), IndexError> {
        // Check B-tree indexes
        let btree = self.btree_indexes.read().expect("TableIndexManager btree_indexes lock poisoned");
        if let Some(index) = btree.get(index_name) {
            return index.insert(key, row_id);
        }
        drop(btree);

        // Check hash indexes
        let hash = self.hash_indexes.read().expect("TableIndexManager hash_indexes lock poisoned");
        if let Some(index) = hash.get(index_name) {
            return index.insert(key, row_id);
        }

        Err(IndexError::IndexNotFound(index_name.to_string()))
    }

    /// Remove a row from a specific index by name.
    pub fn remove_from_index(
        &self,
        index_name: &str,
        key: &IndexKey,
        row_id: RowId,
    ) -> Result<bool, IndexError> {
        // Check B-tree indexes
        let btree = self.btree_indexes.read().expect("TableIndexManager btree_indexes lock poisoned");
        if let Some(index) = btree.get(index_name) {
            return Ok(index.remove(key, row_id));
        }
        drop(btree);

        // Check hash indexes
        let hash = self.hash_indexes.read().expect("TableIndexManager hash_indexes lock poisoned");
        if let Some(index) = hash.get(index_name) {
            return Ok(index.remove(key, row_id));
        }

        Err(IndexError::IndexNotFound(index_name.to_string()))
    }

    fn build_key(&self, columns: &[String], values: &HashMap<String, Value>) -> IndexKey {
        let key_values: Vec<IndexValue> = columns
            .iter()
            .map(|col| {
                values
                    .get(col)
                    .map(IndexKey::from_value)
                    .unwrap_or(IndexValue::Null)
            })
            .collect();
        IndexKey::new(key_values)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btree_index_basic() {
        let index = BTreeIndex::new(
            "idx_test".to_string(),
            "users".to_string(),
            vec!["id".to_string()],
            false,
        );

        // Insert some keys
        index.insert(IndexKey::single(IndexValue::Int(1)), 0).unwrap();
        index.insert(IndexKey::single(IndexValue::Int(2)), 1).unwrap();
        index.insert(IndexKey::single(IndexValue::Int(3)), 2).unwrap();

        // Lookup
        assert_eq!(index.get(&IndexKey::single(IndexValue::Int(2))), vec![1]);
        assert_eq!(index.get(&IndexKey::single(IndexValue::Int(4))), Vec::<RowId>::new());
    }

    #[test]
    fn test_btree_index_range() {
        let index = BTreeIndex::new(
            "idx_test".to_string(),
            "users".to_string(),
            vec!["age".to_string()],
            false,
        );

        for i in 0..10 {
            index.insert(IndexKey::single(IndexValue::Int(i * 10)), i as RowId).unwrap();
        }

        // Range [20, 50]
        let result = index.range(
            Some(&IndexKey::single(IndexValue::Int(20))),
            Some(&IndexKey::single(IndexValue::Int(50))),
            true,
            true,
        );
        assert_eq!(result.len(), 4); // 20, 30, 40, 50

        // Range (20, 50)
        let result = index.range(
            Some(&IndexKey::single(IndexValue::Int(20))),
            Some(&IndexKey::single(IndexValue::Int(50))),
            false,
            false,
        );
        assert_eq!(result.len(), 2); // 30, 40
    }

    #[test]
    fn test_btree_unique_constraint() {
        let index = BTreeIndex::new(
            "idx_unique".to_string(),
            "users".to_string(),
            vec!["email".to_string()],
            true,
        );

        index.insert(IndexKey::single(IndexValue::String("a@b.com".to_string())), 0).unwrap();

        let result = index.insert(IndexKey::single(IndexValue::String("a@b.com".to_string())), 1);
        assert!(matches!(result, Err(IndexError::DuplicateKey(_))));
    }

    #[test]
    fn test_hash_index_basic() {
        let index = HashIndex::new(
            "idx_hash".to_string(),
            "users".to_string(),
            vec!["status".to_string()],
            false,
        );

        index.insert(IndexKey::single(IndexValue::String("active".to_string())), 0).unwrap();
        index.insert(IndexKey::single(IndexValue::String("active".to_string())), 1).unwrap();
        index.insert(IndexKey::single(IndexValue::String("inactive".to_string())), 2).unwrap();

        let active = index.get(&IndexKey::single(IndexValue::String("active".to_string())));
        assert_eq!(active.len(), 2);

        let inactive = index.get(&IndexKey::single(IndexValue::String("inactive".to_string())));
        assert_eq!(inactive.len(), 1);
    }

    #[test]
    fn test_composite_key() {
        let index = BTreeIndex::new(
            "idx_composite".to_string(),
            "orders".to_string(),
            vec!["user_id".to_string(), "order_date".to_string()],
            false,
        );

        let key1 = IndexKey::new(vec![
            IndexValue::Int(1),
            IndexValue::String("2024-01-01".to_string()),
        ]);
        let key2 = IndexKey::new(vec![
            IndexValue::Int(1),
            IndexValue::String("2024-01-02".to_string()),
        ]);
        let key3 = IndexKey::new(vec![
            IndexValue::Int(2),
            IndexValue::String("2024-01-01".to_string()),
        ]);

        index.insert(key1.clone(), 0).unwrap();
        index.insert(key2.clone(), 1).unwrap();
        index.insert(key3.clone(), 2).unwrap();

        assert_eq!(index.get(&key1), vec![0]);
        assert_eq!(index.get(&key2), vec![1]);
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn test_table_index_manager() {
        let manager = TableIndexManager::new("users".to_string());

        // Create indexes
        manager.create_index(
            "idx_id".to_string(),
            vec!["id".to_string()],
            true,
            IndexType::BTree,
        ).unwrap();

        manager.create_index(
            "idx_status".to_string(),
            vec!["status".to_string()],
            false,
            IndexType::Hash,
        ).unwrap();

        // Insert a row
        let mut values = HashMap::new();
        values.insert("id".to_string(), Value::Integer(1));
        values.insert("status".to_string(), Value::String("active".to_string()));

        manager.insert_row(0, &values).unwrap();

        // Lookup
        let key_id = IndexKey::single(IndexValue::Int(1));
        let key_status = IndexKey::single(IndexValue::String("active".to_string()));

        assert_eq!(manager.lookup_eq("idx_id", &key_id), Some(vec![0]));
        assert_eq!(manager.lookup_eq("idx_status", &key_status), Some(vec![0]));
    }

    #[test]
    fn test_find_index_for_columns() {
        let manager = TableIndexManager::new("users".to_string());

        manager.create_index(
            "idx_name".to_string(),
            vec!["name".to_string()],
            false,
            IndexType::Hash,
        ).unwrap();

        manager.create_index(
            "idx_age_city".to_string(),
            vec!["age".to_string(), "city".to_string()],
            false,
            IndexType::BTree,
        ).unwrap();

        // Exact match for hash index
        let result = manager.find_index_for_columns(&["name".to_string()]);
        assert_eq!(result, Some(("idx_name".to_string(), IndexType::Hash)));

        // Prefix match for B-tree index
        let result = manager.find_index_for_columns(&["age".to_string()]);
        assert_eq!(result, Some(("idx_age_city".to_string(), IndexType::BTree)));

        // No matching index
        let result = manager.find_index_for_columns(&["email".to_string()]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_index_remove() {
        let index = BTreeIndex::new(
            "idx_test".to_string(),
            "users".to_string(),
            vec!["id".to_string()],
            false,
        );

        let key = IndexKey::single(IndexValue::Int(1));
        index.insert(key.clone(), 0).unwrap();
        index.insert(key.clone(), 1).unwrap();

        assert_eq!(index.get(&key).len(), 2);

        index.remove(&key, 0);
        assert_eq!(index.get(&key).len(), 1);

        index.remove(&key, 1);
        assert_eq!(index.get(&key).len(), 0);
    }
}
