//! Aegis Replication Log
//!
//! Replicated log for Raft consensus.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::RwLock;

// =============================================================================
// Log Index
// =============================================================================

/// Index in the replicated log.
pub type LogIndex = u64;

/// Term number for Raft consensus.
pub type Term = u64;

// =============================================================================
// Log Entry
// =============================================================================

/// An entry in the replicated log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub index: LogIndex,
    pub term: Term,
    pub entry_type: EntryType,
    pub data: Vec<u8>,
    pub timestamp: u64,
}

impl LogEntry {
    /// Create a new log entry.
    pub fn new(index: LogIndex, term: Term, entry_type: EntryType, data: Vec<u8>) -> Self {
        Self {
            index,
            term,
            entry_type,
            data,
            timestamp: current_timestamp(),
        }
    }

    /// Create a command entry.
    pub fn command(index: LogIndex, term: Term, data: Vec<u8>) -> Self {
        Self::new(index, term, EntryType::Command, data)
    }

    /// Create a configuration change entry.
    pub fn config_change(index: LogIndex, term: Term, data: Vec<u8>) -> Self {
        Self::new(index, term, EntryType::ConfigChange, data)
    }

    /// Create a no-op entry.
    pub fn noop(index: LogIndex, term: Term) -> Self {
        Self::new(index, term, EntryType::NoOp, Vec::new())
    }
}

// =============================================================================
// Entry Type
// =============================================================================

/// Type of log entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryType {
    /// A state machine command.
    Command,
    /// A cluster configuration change.
    ConfigChange,
    /// A no-op entry for leader confirmation.
    NoOp,
}

// =============================================================================
// Replicated Log
// =============================================================================

/// The replicated log for Raft consensus.
pub struct ReplicatedLog {
    entries: RwLock<VecDeque<LogEntry>>,
    first_index: RwLock<LogIndex>,
    commit_index: RwLock<LogIndex>,
    last_applied: RwLock<LogIndex>,
    snapshot_index: RwLock<LogIndex>,
    snapshot_term: RwLock<Term>,
}

impl ReplicatedLog {
    /// Create a new replicated log.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(VecDeque::new()),
            first_index: RwLock::new(1),
            commit_index: RwLock::new(0),
            last_applied: RwLock::new(0),
            snapshot_index: RwLock::new(0),
            snapshot_term: RwLock::new(0),
        }
    }

    /// Append an entry to the log.
    pub fn append(&self, entry: LogEntry) -> LogIndex {
        let mut entries = self.entries.write().expect("log entries lock poisoned");
        let index = entry.index;
        entries.push_back(entry);
        index
    }

    /// Append multiple entries.
    pub fn append_entries(&self, new_entries: Vec<LogEntry>) -> LogIndex {
        let mut entries = self.entries.write().expect("log entries lock poisoned");
        let mut last_index = 0;

        for entry in new_entries {
            last_index = entry.index;
            entries.push_back(entry);
        }

        last_index
    }

    /// Get an entry by index.
    pub fn get(&self, index: LogIndex) -> Option<LogEntry> {
        let entries = self.entries.read().expect("log entries lock poisoned");
        let first = *self.first_index.read().expect("log first_index lock poisoned");

        if index < first {
            return None;
        }

        let offset = (index - first) as usize;
        entries.get(offset).cloned()
    }

    /// Get entries in a range.
    pub fn get_range(&self, start: LogIndex, end: LogIndex) -> Vec<LogEntry> {
        let entries = self.entries.read().expect("log entries lock poisoned");
        let first = *self.first_index.read().expect("log first_index lock poisoned");

        if start < first {
            return Vec::new();
        }

        let start_offset = (start - first) as usize;
        let end_offset = (end - first) as usize;

        entries
            .iter()
            .skip(start_offset)
            .take(end_offset.saturating_sub(start_offset))
            .cloned()
            .collect()
    }

    /// Get the last log index.
    pub fn last_index(&self) -> LogIndex {
        let entries = self.entries.read().expect("log entries lock poisoned");
        let first = *self.first_index.read().expect("log first_index lock poisoned");

        if entries.is_empty() {
            let snapshot = *self.snapshot_index.read().expect("log snapshot_index lock poisoned");
            return snapshot;
        }

        first + entries.len() as u64 - 1
    }

    /// Get the term of the last entry.
    pub fn last_term(&self) -> Term {
        let entries = self.entries.read().expect("log entries lock poisoned");

        if let Some(entry) = entries.back() {
            return entry.term;
        }

        *self.snapshot_term.read().expect("log snapshot_term lock poisoned")
    }

    /// Get the term of an entry at a specific index.
    pub fn term_at(&self, index: LogIndex) -> Option<Term> {
        if index == 0 {
            return Some(0);
        }

        let snapshot_index = *self.snapshot_index.read().expect("log snapshot_index lock poisoned");
        if index == snapshot_index {
            return Some(*self.snapshot_term.read().expect("log snapshot_term lock poisoned"));
        }

        self.get(index).map(|e| e.term)
    }

    /// Get the commit index.
    pub fn commit_index(&self) -> LogIndex {
        *self.commit_index.read().expect("log commit_index lock poisoned")
    }

    /// Set the commit index.
    pub fn set_commit_index(&self, index: LogIndex) {
        let mut commit = self.commit_index.write().expect("log commit_index lock poisoned");
        if index > *commit {
            *commit = index;
        }
    }

    /// Get the last applied index.
    pub fn last_applied(&self) -> LogIndex {
        *self.last_applied.read().expect("log last_applied lock poisoned")
    }

    /// Set the last applied index.
    pub fn set_last_applied(&self, index: LogIndex) {
        let mut applied = self.last_applied.write().expect("log last_applied lock poisoned");
        *applied = index;
    }

    /// Check if there are entries to apply.
    pub fn has_entries_to_apply(&self) -> bool {
        let commit = self.commit_index();
        let applied = self.last_applied();
        commit > applied
    }

    /// Get the next entry to apply.
    pub fn next_to_apply(&self) -> Option<LogEntry> {
        let commit = self.commit_index();
        let applied = self.last_applied();

        if commit > applied {
            self.get(applied + 1)
        } else {
            None
        }
    }

    /// Truncate the log from a given index (inclusive).
    pub fn truncate_from(&self, index: LogIndex) {
        let mut entries = self.entries.write().expect("log entries lock poisoned");
        let first = *self.first_index.read().expect("log first_index lock poisoned");

        if index < first {
            entries.clear();
            return;
        }

        let offset = (index - first) as usize;
        entries.truncate(offset);
    }

    /// Compact the log up to a given index.
    pub fn compact(&self, up_to: LogIndex, term: Term) {
        let mut entries = self.entries.write().expect("log entries lock poisoned");
        let first = *self.first_index.read().expect("log first_index lock poisoned");

        if up_to < first {
            return;
        }

        let remove_count = (up_to - first + 1) as usize;
        for _ in 0..remove_count.min(entries.len()) {
            entries.pop_front();
        }

        *self.first_index.write().expect("log first_index lock poisoned") = up_to + 1;
        *self.snapshot_index.write().expect("log snapshot_index lock poisoned") = up_to;
        *self.snapshot_term.write().expect("log snapshot_term lock poisoned") = term;
    }

    /// Get the length of the log.
    pub fn len(&self) -> usize {
        let entries = self.entries.read().expect("log entries lock poisoned");
        entries.len()
    }

    /// Check if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if a log is up-to-date compared to this log.
    pub fn is_up_to_date(&self, last_log_index: LogIndex, last_log_term: Term) -> bool {
        let our_last_term = self.last_term();
        let our_last_index = self.last_index();

        if last_log_term > our_last_term {
            return true;
        }

        if last_log_term == our_last_term && last_log_index >= our_last_index {
            return true;
        }

        false
    }

    /// Find the conflict point when appending entries.
    pub fn find_conflict(&self, entries: &[LogEntry]) -> Option<LogIndex> {
        for entry in entries {
            if let Some(term) = self.term_at(entry.index) {
                if term != entry.term {
                    return Some(entry.index);
                }
            }
        }
        None
    }
}

impl Default for ReplicatedLog {
    fn default() -> Self {
        Self::new()
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry() {
        let entry = LogEntry::command(1, 1, vec![1, 2, 3]);
        assert_eq!(entry.index, 1);
        assert_eq!(entry.term, 1);
        assert_eq!(entry.entry_type, EntryType::Command);
    }

    #[test]
    fn test_replicated_log() {
        let log = ReplicatedLog::new();

        let entry1 = LogEntry::command(1, 1, vec![1]);
        let entry2 = LogEntry::command(2, 1, vec![2]);
        let entry3 = LogEntry::command(3, 2, vec![3]);

        log.append(entry1);
        log.append(entry2);
        log.append(entry3);

        assert_eq!(log.len(), 3);
        assert_eq!(log.last_index(), 3);
        assert_eq!(log.last_term(), 2);
    }

    #[test]
    fn test_get_entry() {
        let log = ReplicatedLog::new();

        log.append(LogEntry::command(1, 1, vec![1]));
        log.append(LogEntry::command(2, 1, vec![2]));

        let entry = log.get(1).unwrap();
        assert_eq!(entry.index, 1);

        let entry = log.get(2).unwrap();
        assert_eq!(entry.index, 2);

        assert!(log.get(3).is_none());
    }

    #[test]
    fn test_commit_and_apply() {
        let log = ReplicatedLog::new();

        log.append(LogEntry::command(1, 1, vec![1]));
        log.append(LogEntry::command(2, 1, vec![2]));
        log.append(LogEntry::command(3, 1, vec![3]));

        assert_eq!(log.commit_index(), 0);
        assert_eq!(log.last_applied(), 0);

        log.set_commit_index(2);
        assert_eq!(log.commit_index(), 2);
        assert!(log.has_entries_to_apply());

        let entry = log.next_to_apply().unwrap();
        assert_eq!(entry.index, 1);

        log.set_last_applied(1);
        let entry = log.next_to_apply().unwrap();
        assert_eq!(entry.index, 2);
    }

    #[test]
    fn test_truncate() {
        let log = ReplicatedLog::new();

        log.append(LogEntry::command(1, 1, vec![1]));
        log.append(LogEntry::command(2, 1, vec![2]));
        log.append(LogEntry::command(3, 2, vec![3]));

        log.truncate_from(2);
        assert_eq!(log.len(), 1);
        assert_eq!(log.last_index(), 1);
    }

    #[test]
    fn test_is_up_to_date() {
        let log = ReplicatedLog::new();

        log.append(LogEntry::command(1, 1, vec![1]));
        log.append(LogEntry::command(2, 2, vec![2]));

        assert!(log.is_up_to_date(2, 2));
        assert!(log.is_up_to_date(3, 2));
        assert!(log.is_up_to_date(1, 3));
        assert!(!log.is_up_to_date(1, 1));
    }
}
