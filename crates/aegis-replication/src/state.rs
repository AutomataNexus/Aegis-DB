//! Aegis Replication State Machine
//!
//! State machine abstraction for replicated state with pluggable backends.
//! Supports both key-value operations and full database operations (SQL, documents, graph).
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

// =============================================================================
// State Machine Backend Trait
// =============================================================================

/// Trait for pluggable state machine backends.
/// Implementations can delegate to in-memory storage, disk-based storage,
/// or any other backend.
pub trait StateMachineBackend: Send + Sync {
    /// Apply a command to the state machine at the given log index.
    fn apply(&self, command: &Command, index: u64) -> CommandResult;

    /// Get a value by key.
    fn get(&self, key: &str) -> Option<Vec<u8>>;

    /// Get the last applied log index.
    fn last_applied(&self) -> u64;

    /// Get the current version/sequence number.
    fn version(&self) -> u64;

    /// Get the number of keys stored.
    fn len(&self) -> usize;

    /// Check if the state machine is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Take a snapshot of the current state.
    fn snapshot(&self) -> Snapshot;

    /// Restore state from a snapshot.
    fn restore(&self, snapshot: Snapshot);
}

// =============================================================================
// Command
// =============================================================================

/// A command to be applied to the state machine.
/// Supports both key-value and database operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub command_type: CommandType,
    pub key: String,
    pub value: Option<Vec<u8>>,
    pub metadata: HashMap<String, String>,
}

impl Command {
    /// Create a get command.
    pub fn get(key: impl Into<String>) -> Self {
        Self {
            command_type: CommandType::Get,
            key: key.into(),
            value: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a set command.
    pub fn set(key: impl Into<String>, value: Vec<u8>) -> Self {
        Self {
            command_type: CommandType::Set,
            key: key.into(),
            value: Some(value),
            metadata: HashMap::new(),
        }
    }

    /// Create a delete command.
    pub fn delete(key: impl Into<String>) -> Self {
        Self {
            command_type: CommandType::Delete,
            key: key.into(),
            value: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a SQL execution command.
    pub fn sql(statement: impl Into<String>) -> Self {
        Self {
            command_type: CommandType::SqlExecute,
            key: String::new(),
            value: Some(statement.into().into_bytes()),
            metadata: HashMap::new(),
        }
    }

    /// Create a document insert command.
    pub fn document_insert(collection: impl Into<String>, document: Vec<u8>) -> Self {
        Self {
            command_type: CommandType::DocumentInsert,
            key: collection.into(),
            value: Some(document),
            metadata: HashMap::new(),
        }
    }

    /// Create a document update command.
    pub fn document_update(collection: impl Into<String>, doc_id: impl Into<String>, document: Vec<u8>) -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("doc_id".to_string(), doc_id.into());
        Self {
            command_type: CommandType::DocumentUpdate,
            key: collection.into(),
            value: Some(document),
            metadata,
        }
    }

    /// Create a document delete command.
    pub fn document_delete(collection: impl Into<String>, doc_id: impl Into<String>) -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("doc_id".to_string(), doc_id.into());
        Self {
            command_type: CommandType::DocumentDelete,
            key: collection.into(),
            value: None,
            metadata,
        }
    }

    /// Create a graph node create command.
    pub fn graph_create_node(label: impl Into<String>, properties: Vec<u8>) -> Self {
        Self {
            command_type: CommandType::GraphCreateNode,
            key: label.into(),
            value: Some(properties),
            metadata: HashMap::new(),
        }
    }

    /// Create a graph edge create command.
    pub fn graph_create_edge(edge_type: impl Into<String>, from_node: impl Into<String>, to_node: impl Into<String>, properties: Vec<u8>) -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("from".to_string(), from_node.into());
        metadata.insert("to".to_string(), to_node.into());
        Self {
            command_type: CommandType::GraphCreateEdge,
            key: edge_type.into(),
            value: Some(properties),
            metadata,
        }
    }

    /// Create a transaction begin command.
    pub fn tx_begin(tx_id: u64) -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("tx_id".to_string(), tx_id.to_string());
        Self {
            command_type: CommandType::TransactionBegin,
            key: String::new(),
            value: None,
            metadata,
        }
    }

    /// Create a transaction commit command.
    pub fn tx_commit(tx_id: u64) -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("tx_id".to_string(), tx_id.to_string());
        Self {
            command_type: CommandType::TransactionCommit,
            key: String::new(),
            value: None,
            metadata,
        }
    }

    /// Create a transaction rollback command.
    pub fn tx_rollback(tx_id: u64) -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("tx_id".to_string(), tx_id.to_string());
        Self {
            command_type: CommandType::TransactionRollback,
            key: String::new(),
            value: None,
            metadata,
        }
    }

    /// Serialize the command.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserialize a command.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }

    /// Check if this command is a write operation (modifies state).
    pub fn is_write(&self) -> bool {
        !matches!(self.command_type, CommandType::Get)
    }

    /// Get the SQL statement if this is a SQL command.
    pub fn sql_statement(&self) -> Option<&str> {
        if self.command_type == CommandType::SqlExecute {
            self.value.as_ref().and_then(|v| std::str::from_utf8(v).ok())
        } else {
            None
        }
    }

    /// Get the transaction ID if this is a transaction command.
    pub fn transaction_id(&self) -> Option<u64> {
        self.metadata.get("tx_id").and_then(|s| s.parse().ok())
    }
}

// =============================================================================
// Command Type
// =============================================================================

/// Type of command - supports key-value, SQL, document, graph, and transaction operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandType {
    // Key-value operations
    Get,
    Set,
    Delete,
    CompareAndSwap,
    Increment,

    // SQL operations
    SqlExecute,

    // Document operations
    DocumentInsert,
    DocumentUpdate,
    DocumentDelete,

    // Graph operations
    GraphCreateNode,
    GraphDeleteNode,
    GraphCreateEdge,
    GraphDeleteEdge,

    // Transaction control
    TransactionBegin,
    TransactionCommit,
    TransactionRollback,

    // Generic custom operation
    Custom,
}

// =============================================================================
// Command Result
// =============================================================================

/// Result of applying a command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub value: Option<Vec<u8>>,
    pub error: Option<String>,
    pub applied_index: u64,
}

impl CommandResult {
    pub fn success(value: Option<Vec<u8>>, applied_index: u64) -> Self {
        Self {
            success: true,
            value,
            error: None,
            applied_index,
        }
    }

    pub fn error(message: impl Into<String>, applied_index: u64) -> Self {
        Self {
            success: false,
            value: None,
            error: Some(message.into()),
            applied_index,
        }
    }
}

// =============================================================================
// State Machine (In-Memory Implementation)
// =============================================================================

/// The replicated state machine with in-memory storage.
/// For production use, consider using DatabaseStateMachine which persists to storage.
pub struct StateMachine {
    data: RwLock<HashMap<String, Vec<u8>>>,
    last_applied: RwLock<u64>,
    version: RwLock<u64>,
}

impl StateMachine {
    /// Create a new in-memory state machine.
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            last_applied: RwLock::new(0),
            version: RwLock::new(0),
        }
    }

    fn apply_kv_command(&self, command: &Command, index: u64, data: &mut HashMap<String, Vec<u8>>, version: &mut u64) -> CommandResult {
        match command.command_type {
            CommandType::Get => {
                let value = data.get(&command.key).cloned();
                CommandResult::success(value, index)
            }
            CommandType::Set => {
                if let Some(ref value) = command.value {
                    data.insert(command.key.clone(), value.clone());
                    *version += 1;
                    CommandResult::success(None, index)
                } else {
                    CommandResult::error("No value provided", index)
                }
            }
            CommandType::Delete => {
                let old = data.remove(&command.key);
                *version += 1;
                CommandResult::success(old, index)
            }
            CommandType::CompareAndSwap => {
                // Expected value is in metadata["expected"]
                let expected = command.metadata.get("expected").map(|s| s.as_bytes().to_vec());
                let current = data.get(&command.key).cloned();

                if current == expected {
                    if let Some(ref new_value) = command.value {
                        data.insert(command.key.clone(), new_value.clone());
                        *version += 1;
                        CommandResult::success(Some(b"true".to_vec()), index)
                    } else {
                        CommandResult::error("No new value provided", index)
                    }
                } else {
                    CommandResult::success(Some(b"false".to_vec()), index)
                }
            }
            CommandType::Increment => {
                let current = data
                    .get(&command.key)
                    .and_then(|v| String::from_utf8(v.clone()).ok())
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(0);

                let new_value = (current + 1).to_string().into_bytes();
                data.insert(command.key.clone(), new_value.clone());
                *version += 1;
                CommandResult::success(Some(new_value), index)
            }
            _ => CommandResult::error(
                format!("Command type {:?} not supported by in-memory state machine", command.command_type),
                index
            ),
        }
    }
}

impl StateMachineBackend for StateMachine {
    fn apply(&self, command: &Command, index: u64) -> CommandResult {
        let mut data = self.data.write().expect("state machine data lock poisoned");
        let mut last_applied = self.last_applied.write().expect("state machine last_applied lock poisoned");
        let mut version = self.version.write().expect("state machine version lock poisoned");

        if index <= *last_applied {
            return CommandResult::error("Already applied", *last_applied);
        }

        let result = self.apply_kv_command(command, index, &mut data, &mut version);
        *last_applied = index;
        result
    }

    fn get(&self, key: &str) -> Option<Vec<u8>> {
        let data = self.data.read().expect("state machine data lock poisoned");
        data.get(key).cloned()
    }

    fn last_applied(&self) -> u64 {
        *self.last_applied.read().expect("state machine last_applied lock poisoned")
    }

    fn version(&self) -> u64 {
        *self.version.read().expect("state machine version lock poisoned")
    }

    fn len(&self) -> usize {
        let data = self.data.read().expect("state machine data lock poisoned");
        data.len()
    }

    fn snapshot(&self) -> Snapshot {
        let data = self.data.read().expect("state machine data lock poisoned");
        let last_applied = *self.last_applied.read().expect("state machine last_applied lock poisoned");
        let version = *self.version.read().expect("state machine version lock poisoned");

        Snapshot {
            data: data.clone(),
            last_applied,
            version,
        }
    }

    fn restore(&self, snapshot: Snapshot) {
        let mut data = self.data.write().expect("state machine data lock poisoned");
        let mut last_applied = self.last_applied.write().expect("state machine last_applied lock poisoned");
        let mut version = self.version.write().expect("state machine version lock poisoned");

        *data = snapshot.data;
        *last_applied = snapshot.last_applied;
        *version = snapshot.version;
    }
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Database State Machine (Storage-backed)
// =============================================================================

/// A state machine that delegates operations to the actual database storage.
/// This allows Raft to replicate real database operations.
pub struct DatabaseStateMachine<F: DatabaseOperationHandler> {
    handler: F,
    last_applied: RwLock<u64>,
    version: RwLock<u64>,
    /// Cached key-value data for snapshot/restore and simple KV operations
    kv_cache: RwLock<HashMap<String, Vec<u8>>>,
}

/// Trait for handling database operations.
/// Implement this to connect Raft replication to your storage layer.
pub trait DatabaseOperationHandler: Send + Sync {
    /// Execute a SQL statement.
    fn execute_sql(&self, sql: &str) -> Result<Vec<u8>, String>;

    /// Insert a document into a collection.
    fn insert_document(&self, collection: &str, document: &[u8]) -> Result<String, String>;

    /// Update a document in a collection.
    fn update_document(&self, collection: &str, doc_id: &str, document: &[u8]) -> Result<(), String>;

    /// Delete a document from a collection.
    fn delete_document(&self, collection: &str, doc_id: &str) -> Result<(), String>;

    /// Create a graph node.
    fn create_node(&self, label: &str, properties: &[u8]) -> Result<String, String>;

    /// Delete a graph node.
    fn delete_node(&self, node_id: &str) -> Result<(), String>;

    /// Create a graph edge.
    fn create_edge(&self, edge_type: &str, from_node: &str, to_node: &str, properties: &[u8]) -> Result<String, String>;

    /// Delete a graph edge.
    fn delete_edge(&self, edge_id: &str) -> Result<(), String>;

    /// Begin a transaction.
    fn begin_transaction(&self, tx_id: u64) -> Result<(), String>;

    /// Commit a transaction.
    fn commit_transaction(&self, tx_id: u64) -> Result<(), String>;

    /// Rollback a transaction.
    fn rollback_transaction(&self, tx_id: u64) -> Result<(), String>;

    /// Create a full snapshot of the database state.
    fn create_snapshot(&self) -> Result<Vec<u8>, String>;

    /// Restore from a snapshot.
    fn restore_snapshot(&self, data: &[u8]) -> Result<(), String>;
}

impl<F: DatabaseOperationHandler> DatabaseStateMachine<F> {
    /// Create a new database state machine with the given operation handler.
    pub fn new(handler: F) -> Self {
        Self {
            handler,
            last_applied: RwLock::new(0),
            version: RwLock::new(0),
            kv_cache: RwLock::new(HashMap::new()),
        }
    }

    fn apply_database_command(&self, command: &Command, index: u64) -> CommandResult {
        match command.command_type {
            // Key-value operations use the cache
            CommandType::Get => {
                let cache = self.kv_cache.read().expect("database state machine kv_cache lock poisoned");
                let value = cache.get(&command.key).cloned();
                CommandResult::success(value, index)
            }
            CommandType::Set => {
                if let Some(ref value) = command.value {
                    let mut cache = self.kv_cache.write().expect("database state machine kv_cache lock poisoned");
                    cache.insert(command.key.clone(), value.clone());
                    CommandResult::success(None, index)
                } else {
                    CommandResult::error("No value provided", index)
                }
            }
            CommandType::Delete => {
                let mut cache = self.kv_cache.write().expect("database state machine kv_cache lock poisoned");
                let old = cache.remove(&command.key);
                CommandResult::success(old, index)
            }

            // SQL operations
            CommandType::SqlExecute => {
                if let Some(sql) = command.sql_statement() {
                    match self.handler.execute_sql(sql) {
                        Ok(result) => CommandResult::success(Some(result), index),
                        Err(e) => CommandResult::error(e, index),
                    }
                } else {
                    CommandResult::error("No SQL statement provided", index)
                }
            }

            // Document operations
            CommandType::DocumentInsert => {
                if let Some(ref doc) = command.value {
                    match self.handler.insert_document(&command.key, doc) {
                        Ok(doc_id) => CommandResult::success(Some(doc_id.into_bytes()), index),
                        Err(e) => CommandResult::error(e, index),
                    }
                } else {
                    CommandResult::error("No document provided", index)
                }
            }
            CommandType::DocumentUpdate => {
                if let (Some(doc_id), Some(ref doc)) = (command.metadata.get("doc_id"), &command.value) {
                    match self.handler.update_document(&command.key, doc_id, doc) {
                        Ok(()) => CommandResult::success(None, index),
                        Err(e) => CommandResult::error(e, index),
                    }
                } else {
                    CommandResult::error("Missing doc_id or document", index)
                }
            }
            CommandType::DocumentDelete => {
                if let Some(doc_id) = command.metadata.get("doc_id") {
                    match self.handler.delete_document(&command.key, doc_id) {
                        Ok(()) => CommandResult::success(None, index),
                        Err(e) => CommandResult::error(e, index),
                    }
                } else {
                    CommandResult::error("Missing doc_id", index)
                }
            }

            // Graph operations
            CommandType::GraphCreateNode => {
                if let Some(ref props) = command.value {
                    match self.handler.create_node(&command.key, props) {
                        Ok(node_id) => CommandResult::success(Some(node_id.into_bytes()), index),
                        Err(e) => CommandResult::error(e, index),
                    }
                } else {
                    CommandResult::error("No properties provided", index)
                }
            }
            CommandType::GraphDeleteNode => {
                match self.handler.delete_node(&command.key) {
                    Ok(()) => CommandResult::success(None, index),
                    Err(e) => CommandResult::error(e, index),
                }
            }
            CommandType::GraphCreateEdge => {
                if let (Some(from), Some(to), Some(ref props)) = (
                    command.metadata.get("from"),
                    command.metadata.get("to"),
                    &command.value,
                ) {
                    match self.handler.create_edge(&command.key, from, to, props) {
                        Ok(edge_id) => CommandResult::success(Some(edge_id.into_bytes()), index),
                        Err(e) => CommandResult::error(e, index),
                    }
                } else {
                    CommandResult::error("Missing from, to, or properties", index)
                }
            }
            CommandType::GraphDeleteEdge => {
                match self.handler.delete_edge(&command.key) {
                    Ok(()) => CommandResult::success(None, index),
                    Err(e) => CommandResult::error(e, index),
                }
            }

            // Transaction operations
            CommandType::TransactionBegin => {
                if let Some(tx_id) = command.transaction_id() {
                    match self.handler.begin_transaction(tx_id) {
                        Ok(()) => CommandResult::success(None, index),
                        Err(e) => CommandResult::error(e, index),
                    }
                } else {
                    CommandResult::error("Missing transaction ID", index)
                }
            }
            CommandType::TransactionCommit => {
                if let Some(tx_id) = command.transaction_id() {
                    match self.handler.commit_transaction(tx_id) {
                        Ok(()) => CommandResult::success(None, index),
                        Err(e) => CommandResult::error(e, index),
                    }
                } else {
                    CommandResult::error("Missing transaction ID", index)
                }
            }
            CommandType::TransactionRollback => {
                if let Some(tx_id) = command.transaction_id() {
                    match self.handler.rollback_transaction(tx_id) {
                        Ok(()) => CommandResult::success(None, index),
                        Err(e) => CommandResult::error(e, index),
                    }
                } else {
                    CommandResult::error("Missing transaction ID", index)
                }
            }

            // Other commands
            CommandType::CompareAndSwap | CommandType::Increment => {
                CommandResult::error("Use key-value state machine for these operations", index)
            }
            CommandType::Custom => {
                CommandResult::error("Custom commands not handled", index)
            }
        }
    }
}

impl<F: DatabaseOperationHandler> StateMachineBackend for DatabaseStateMachine<F> {
    fn apply(&self, command: &Command, index: u64) -> CommandResult {
        let mut last_applied = self.last_applied.write().expect("database state machine last_applied lock poisoned");
        let mut version = self.version.write().expect("database state machine version lock poisoned");

        if index <= *last_applied {
            return CommandResult::error("Already applied", *last_applied);
        }

        let result = self.apply_database_command(command, index);

        if result.success {
            *version += 1;
        }
        *last_applied = index;

        result
    }

    fn get(&self, key: &str) -> Option<Vec<u8>> {
        let cache = self.kv_cache.read().expect("database state machine kv_cache lock poisoned");
        cache.get(key).cloned()
    }

    fn last_applied(&self) -> u64 {
        *self.last_applied.read().expect("database state machine last_applied lock poisoned")
    }

    fn version(&self) -> u64 {
        *self.version.read().expect("database state machine version lock poisoned")
    }

    fn len(&self) -> usize {
        let cache = self.kv_cache.read().expect("database state machine kv_cache lock poisoned");
        cache.len()
    }

    fn snapshot(&self) -> Snapshot {
        // For database state machine, snapshot includes both KV cache and database state
        let cache = self.kv_cache.read().expect("database state machine kv_cache lock poisoned");
        let last_applied = *self.last_applied.read().expect("database state machine last_applied lock poisoned");
        let version = *self.version.read().expect("database state machine version lock poisoned");

        // Try to include database snapshot in the data
        let mut data = cache.clone();
        if let Ok(db_snapshot) = self.handler.create_snapshot() {
            data.insert("__db_snapshot__".to_string(), db_snapshot);
        }

        Snapshot {
            data,
            last_applied,
            version,
        }
    }

    fn restore(&self, snapshot: Snapshot) {
        let mut cache = self.kv_cache.write().expect("database state machine kv_cache lock poisoned");
        let mut last_applied = self.last_applied.write().expect("database state machine last_applied lock poisoned");
        let mut version = self.version.write().expect("database state machine version lock poisoned");

        // Restore database state if present
        if let Some(db_snapshot) = snapshot.data.get("__db_snapshot__") {
            let _ = self.handler.restore_snapshot(db_snapshot);
        }

        // Restore KV cache (excluding the special key)
        *cache = snapshot.data.into_iter()
            .filter(|(k, _)| k != "__db_snapshot__")
            .collect();
        *last_applied = snapshot.last_applied;
        *version = snapshot.version;
    }
}

// =============================================================================
// No-op Database Handler (for testing)
// =============================================================================

/// A no-op handler that logs operations but doesn't persist anything.
/// Useful for testing Raft replication without a real database.
#[derive(Default)]
pub struct NoOpDatabaseHandler;

impl DatabaseOperationHandler for NoOpDatabaseHandler {
    fn execute_sql(&self, _sql: &str) -> Result<Vec<u8>, String> {
        Ok(b"OK".to_vec())
    }

    fn insert_document(&self, _collection: &str, _document: &[u8]) -> Result<String, String> {
        Ok("doc-001".to_string())
    }

    fn update_document(&self, _collection: &str, _doc_id: &str, _document: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn delete_document(&self, _collection: &str, _doc_id: &str) -> Result<(), String> {
        Ok(())
    }

    fn create_node(&self, _label: &str, _properties: &[u8]) -> Result<String, String> {
        Ok("node-001".to_string())
    }

    fn delete_node(&self, _node_id: &str) -> Result<(), String> {
        Ok(())
    }

    fn create_edge(&self, _edge_type: &str, _from_node: &str, _to_node: &str, _properties: &[u8]) -> Result<String, String> {
        Ok("edge-001".to_string())
    }

    fn delete_edge(&self, _edge_id: &str) -> Result<(), String> {
        Ok(())
    }

    fn begin_transaction(&self, _tx_id: u64) -> Result<(), String> {
        Ok(())
    }

    fn commit_transaction(&self, _tx_id: u64) -> Result<(), String> {
        Ok(())
    }

    fn rollback_transaction(&self, _tx_id: u64) -> Result<(), String> {
        Ok(())
    }

    fn create_snapshot(&self) -> Result<Vec<u8>, String> {
        Ok(Vec::new())
    }

    fn restore_snapshot(&self, _data: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

// =============================================================================
// Snapshot
// =============================================================================

/// A snapshot of the state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub data: HashMap<String, Vec<u8>>,
    pub last_applied: u64,
    pub version: u64,
}

impl Snapshot {
    /// Serialize the snapshot.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserialize a snapshot.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command() {
        let cmd = Command::set("key1", b"value1".to_vec());
        assert_eq!(cmd.command_type, CommandType::Set);
        assert_eq!(cmd.key, "key1");

        let bytes = cmd.to_bytes();
        let restored = Command::from_bytes(&bytes).unwrap();
        assert_eq!(restored.key, "key1");
    }

    #[test]
    fn test_state_machine_set_get() {
        let sm = StateMachine::new();

        let cmd = Command::set("key1", b"value1".to_vec());
        let result = sm.apply(&cmd, 1);
        assert!(result.success);
        assert_eq!(sm.last_applied(), 1);

        let value = sm.get("key1").unwrap();
        assert_eq!(value, b"value1");
    }

    #[test]
    fn test_state_machine_delete() {
        let sm = StateMachine::new();

        sm.apply(&Command::set("key1", b"value1".to_vec()), 1);
        assert!(sm.get("key1").is_some());

        sm.apply(&Command::delete("key1"), 2);
        assert!(sm.get("key1").is_none());
    }

    #[test]
    fn test_state_machine_increment() {
        let sm = StateMachine::new();

        sm.apply(&Command::set("counter", b"0".to_vec()), 1);

        let cmd = Command {
            command_type: CommandType::Increment,
            key: "counter".to_string(),
            value: None,
            metadata: HashMap::new(),
        };

        sm.apply(&cmd, 2);
        let value = sm.get("counter").unwrap();
        assert_eq!(String::from_utf8(value).unwrap(), "1");
    }

    #[test]
    fn test_snapshot() {
        let sm = StateMachine::new();

        sm.apply(&Command::set("key1", b"value1".to_vec()), 1);
        sm.apply(&Command::set("key2", b"value2".to_vec()), 2);

        let snapshot = sm.snapshot();
        assert_eq!(snapshot.last_applied, 2);
        assert_eq!(snapshot.data.len(), 2);

        let new_sm = StateMachine::new();
        new_sm.restore(snapshot);

        assert_eq!(new_sm.get("key1").unwrap(), b"value1");
        assert_eq!(new_sm.get("key2").unwrap(), b"value2");
        assert_eq!(new_sm.last_applied(), 2);
    }

    #[test]
    fn test_duplicate_apply() {
        let sm = StateMachine::new();

        let result = sm.apply(&Command::set("key1", b"v1".to_vec()), 1);
        assert!(result.success);

        let result = sm.apply(&Command::set("key1", b"v2".to_vec()), 1);
        assert!(!result.success);

        assert_eq!(sm.get("key1").unwrap(), b"v1");
    }

    #[test]
    fn test_sql_command() {
        let cmd = Command::sql("INSERT INTO users (name) VALUES ('test')");
        assert_eq!(cmd.command_type, CommandType::SqlExecute);
        assert_eq!(cmd.sql_statement(), Some("INSERT INTO users (name) VALUES ('test')"));
        assert!(cmd.is_write());
    }

    #[test]
    fn test_document_commands() {
        let insert = Command::document_insert("users", b"{\"name\": \"test\"}".to_vec());
        assert_eq!(insert.command_type, CommandType::DocumentInsert);
        assert_eq!(insert.key, "users");

        let update = Command::document_update("users", "doc123", b"{\"name\": \"updated\"}".to_vec());
        assert_eq!(update.command_type, CommandType::DocumentUpdate);
        assert_eq!(update.metadata.get("doc_id"), Some(&"doc123".to_string()));

        let delete = Command::document_delete("users", "doc123");
        assert_eq!(delete.command_type, CommandType::DocumentDelete);
    }

    #[test]
    fn test_graph_commands() {
        let node = Command::graph_create_node("Person", b"{\"name\": \"Alice\"}".to_vec());
        assert_eq!(node.command_type, CommandType::GraphCreateNode);
        assert_eq!(node.key, "Person");

        let edge = Command::graph_create_edge("KNOWS", "node1", "node2", b"{}".to_vec());
        assert_eq!(edge.command_type, CommandType::GraphCreateEdge);
        assert_eq!(edge.metadata.get("from"), Some(&"node1".to_string()));
        assert_eq!(edge.metadata.get("to"), Some(&"node2".to_string()));
    }

    #[test]
    fn test_transaction_commands() {
        let begin = Command::tx_begin(123);
        assert_eq!(begin.command_type, CommandType::TransactionBegin);
        assert_eq!(begin.transaction_id(), Some(123));

        let commit = Command::tx_commit(123);
        assert_eq!(commit.command_type, CommandType::TransactionCommit);

        let rollback = Command::tx_rollback(123);
        assert_eq!(rollback.command_type, CommandType::TransactionRollback);
    }

    #[test]
    fn test_database_state_machine() {
        let handler = NoOpDatabaseHandler;
        let sm = DatabaseStateMachine::new(handler);

        // SQL command
        let sql_cmd = Command::sql("INSERT INTO test VALUES (1)");
        let result = sm.apply(&sql_cmd, 1);
        assert!(result.success);
        assert_eq!(result.value, Some(b"OK".to_vec()));

        // Document insert
        let doc_cmd = Command::document_insert("users", b"{}".to_vec());
        let result = sm.apply(&doc_cmd, 2);
        assert!(result.success);
        assert_eq!(result.value, Some(b"doc-001".to_vec()));

        // Transaction
        let tx_begin = Command::tx_begin(1);
        let result = sm.apply(&tx_begin, 3);
        assert!(result.success);

        let tx_commit = Command::tx_commit(1);
        let result = sm.apply(&tx_commit, 4);
        assert!(result.success);

        assert_eq!(sm.last_applied(), 4);
    }

    #[test]
    fn test_database_state_machine_snapshot() {
        let handler = NoOpDatabaseHandler;
        let sm = DatabaseStateMachine::new(handler);

        // Add some KV data
        sm.apply(&Command::set("key1", b"value1".to_vec()), 1);
        sm.apply(&Command::set("key2", b"value2".to_vec()), 2);

        let snapshot = sm.snapshot();
        assert_eq!(snapshot.last_applied, 2);

        // Restore to new state machine
        let handler2 = NoOpDatabaseHandler;
        let sm2 = DatabaseStateMachine::new(handler2);
        sm2.restore(snapshot);

        assert_eq!(sm2.get("key1"), Some(b"value1".to_vec()));
        assert_eq!(sm2.get("key2"), Some(b"value2".to_vec()));
        assert_eq!(sm2.last_applied(), 2);
    }

    #[test]
    fn test_compare_and_swap() {
        let sm = StateMachine::new();

        // Set initial value
        sm.apply(&Command::set("key1", b"old".to_vec()), 1);

        // CAS with correct expected value
        let mut cmd = Command {
            command_type: CommandType::CompareAndSwap,
            key: "key1".to_string(),
            value: Some(b"new".to_vec()),
            metadata: HashMap::new(),
        };
        cmd.metadata.insert("expected".to_string(), "old".to_string());

        let result = sm.apply(&cmd, 2);
        assert!(result.success);
        assert_eq!(result.value, Some(b"true".to_vec()));
        assert_eq!(sm.get("key1"), Some(b"new".to_vec()));

        // CAS with wrong expected value
        cmd.metadata.insert("expected".to_string(), "wrong".to_string());
        cmd.value = Some(b"newer".to_vec());
        let result = sm.apply(&cmd, 3);
        assert!(result.success);
        assert_eq!(result.value, Some(b"false".to_vec()));
        assert_eq!(sm.get("key1"), Some(b"new".to_vec())); // Unchanged
    }
}
