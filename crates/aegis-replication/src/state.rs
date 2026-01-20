//! Aegis Replication State Machine
//!
//! State machine abstraction for replicated state.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

// =============================================================================
// Command
// =============================================================================

/// A command to be applied to the state machine.
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

    /// Serialize the command.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserialize a command.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

// =============================================================================
// Command Type
// =============================================================================

/// Type of command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandType {
    Get,
    Set,
    Delete,
    CompareAndSwap,
    Increment,
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
// State Machine
// =============================================================================

/// The replicated state machine.
pub struct StateMachine {
    data: RwLock<HashMap<String, Vec<u8>>>,
    last_applied: RwLock<u64>,
    version: RwLock<u64>,
}

impl StateMachine {
    /// Create a new state machine.
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
            last_applied: RwLock::new(0),
            version: RwLock::new(0),
        }
    }

    /// Apply a command to the state machine.
    pub fn apply(&self, command: &Command, index: u64) -> CommandResult {
        let mut data = self.data.write().unwrap();
        let mut last_applied = self.last_applied.write().unwrap();
        let mut version = self.version.write().unwrap();

        if index <= *last_applied {
            return CommandResult::error("Already applied", *last_applied);
        }

        let result = match command.command_type {
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
                CommandResult::error("Not implemented", index)
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
            CommandType::Custom => {
                CommandResult::error("Custom commands not handled", index)
            }
        };

        *last_applied = index;
        result
    }

    /// Get a value from the state machine.
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        let data = self.data.read().unwrap();
        data.get(key).cloned()
    }

    /// Get the last applied index.
    pub fn last_applied(&self) -> u64 {
        *self.last_applied.read().unwrap()
    }

    /// Get the current version.
    pub fn version(&self) -> u64 {
        *self.version.read().unwrap()
    }

    /// Get the number of keys.
    pub fn len(&self) -> usize {
        let data = self.data.read().unwrap();
        data.len()
    }

    /// Check if the state machine is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Take a snapshot of the state.
    pub fn snapshot(&self) -> Snapshot {
        let data = self.data.read().unwrap();
        let last_applied = *self.last_applied.read().unwrap();
        let version = *self.version.read().unwrap();

        Snapshot {
            data: data.clone(),
            last_applied,
            version,
        }
    }

    /// Restore from a snapshot.
    pub fn restore(&self, snapshot: Snapshot) {
        let mut data = self.data.write().unwrap();
        let mut last_applied = self.last_applied.write().unwrap();
        let mut version = self.version.write().unwrap();

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
}
