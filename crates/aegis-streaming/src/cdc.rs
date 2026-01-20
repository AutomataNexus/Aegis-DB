//! Aegis Streaming CDC (Change Data Capture)
//!
//! Change data capture for tracking database changes.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::event::{Event, EventData, EventType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// Change Type
// =============================================================================

/// Type of data change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    Insert,
    Update,
    Delete,
    Truncate,
}

impl From<ChangeType> for EventType {
    fn from(ct: ChangeType) -> Self {
        match ct {
            ChangeType::Insert => EventType::Created,
            ChangeType::Update => EventType::Updated,
            ChangeType::Delete => EventType::Deleted,
            ChangeType::Truncate => EventType::Custom("truncate".to_string()),
        }
    }
}

// =============================================================================
// Change Event
// =============================================================================

/// A change data capture event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub change_type: ChangeType,
    pub source: ChangeSource,
    pub timestamp: u64,
    pub key: Option<String>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub metadata: HashMap<String, String>,
}

impl ChangeEvent {
    /// Create an insert change event.
    pub fn insert(source: ChangeSource, key: String, data: serde_json::Value) -> Self {
        Self {
            change_type: ChangeType::Insert,
            source,
            timestamp: current_timestamp(),
            key: Some(key),
            before: None,
            after: Some(data),
            metadata: HashMap::new(),
        }
    }

    /// Create an update change event.
    pub fn update(
        source: ChangeSource,
        key: String,
        before: serde_json::Value,
        after: serde_json::Value,
    ) -> Self {
        Self {
            change_type: ChangeType::Update,
            source,
            timestamp: current_timestamp(),
            key: Some(key),
            before: Some(before),
            after: Some(after),
            metadata: HashMap::new(),
        }
    }

    /// Create a delete change event.
    pub fn delete(source: ChangeSource, key: String, data: serde_json::Value) -> Self {
        Self {
            change_type: ChangeType::Delete,
            source,
            timestamp: current_timestamp(),
            key: Some(key),
            before: Some(data),
            after: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a truncate change event.
    pub fn truncate(source: ChangeSource) -> Self {
        Self {
            change_type: ChangeType::Truncate,
            source,
            timestamp: current_timestamp(),
            key: None,
            before: None,
            after: None,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the change event.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Convert to a generic Event.
    pub fn to_event(&self) -> Event {
        let source = format!("{}.{}", self.source.database, self.source.table);
        let data = serde_json::json!({
            "change_type": format!("{:?}", self.change_type),
            "key": self.key,
            "before": self.before,
            "after": self.after,
        });

        Event::new(self.change_type.into(), source, EventData::Json(data))
    }
}

// =============================================================================
// Change Source
// =============================================================================

/// Source of a change event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSource {
    pub database: String,
    pub table: String,
    pub schema: Option<String>,
}

impl ChangeSource {
    pub fn new(database: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            database: database.into(),
            table: table.into(),
            schema: None,
        }
    }

    pub fn with_schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }

    pub fn full_name(&self) -> String {
        match &self.schema {
            Some(s) => format!("{}.{}.{}", self.database, s, self.table),
            None => format!("{}.{}", self.database, self.table),
        }
    }
}

// =============================================================================
// CDC Configuration
// =============================================================================

/// Configuration for change data capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdcConfig {
    pub enabled: bool,
    pub sources: Vec<CdcSourceConfig>,
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub include_before: bool,
    pub include_schema: bool,
}

impl Default for CdcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sources: Vec::new(),
            batch_size: 100,
            batch_timeout_ms: 1000,
            include_before: true,
            include_schema: false,
        }
    }
}

impl CdcConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_source(&mut self, source: CdcSourceConfig) {
        self.sources.push(source);
    }

    pub fn with_source(mut self, source: CdcSourceConfig) -> Self {
        self.sources.push(source);
        self
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub fn with_batch_timeout(mut self, timeout_ms: u64) -> Self {
        self.batch_timeout_ms = timeout_ms;
        self
    }
}

// =============================================================================
// CDC Source Configuration
// =============================================================================

/// Configuration for a CDC source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdcSourceConfig {
    pub source: ChangeSource,
    pub change_types: Vec<ChangeType>,
    pub columns: Option<Vec<String>>,
    pub filter: Option<String>,
}

impl CdcSourceConfig {
    pub fn new(database: impl Into<String>, table: impl Into<String>) -> Self {
        Self {
            source: ChangeSource::new(database, table),
            change_types: vec![ChangeType::Insert, ChangeType::Update, ChangeType::Delete],
            columns: None,
            filter: None,
        }
    }

    pub fn inserts_only(mut self) -> Self {
        self.change_types = vec![ChangeType::Insert];
        self
    }

    pub fn updates_only(mut self) -> Self {
        self.change_types = vec![ChangeType::Update];
        self
    }

    pub fn deletes_only(mut self) -> Self {
        self.change_types = vec![ChangeType::Delete];
        self
    }

    pub fn with_columns(mut self, columns: Vec<String>) -> Self {
        self.columns = Some(columns);
        self
    }

    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    pub fn should_capture(&self, change_type: ChangeType) -> bool {
        self.change_types.contains(&change_type)
    }
}

// =============================================================================
// CDC Tracker
// =============================================================================

/// Tracks CDC position for resumable streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdcPosition {
    pub lsn: Option<u64>,
    pub timestamp: u64,
    pub sequence: u64,
}

impl CdcPosition {
    pub fn new() -> Self {
        Self {
            lsn: None,
            timestamp: current_timestamp(),
            sequence: 0,
        }
    }

    pub fn with_lsn(mut self, lsn: u64) -> Self {
        self.lsn = Some(lsn);
        self
    }

    pub fn advance(&mut self) {
        self.sequence += 1;
        self.timestamp = current_timestamp();
    }
}

impl Default for CdcPosition {
    fn default() -> Self {
        Self::new()
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
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
    fn test_change_event_insert() {
        let source = ChangeSource::new("mydb", "users");
        let data = serde_json::json!({"id": 1, "name": "Alice"});

        let event = ChangeEvent::insert(source, "1".to_string(), data);

        assert_eq!(event.change_type, ChangeType::Insert);
        assert!(event.before.is_none());
        assert!(event.after.is_some());
    }

    #[test]
    fn test_change_event_update() {
        let source = ChangeSource::new("mydb", "users");
        let before = serde_json::json!({"id": 1, "name": "Alice"});
        let after = serde_json::json!({"id": 1, "name": "Alice Smith"});

        let event = ChangeEvent::update(source, "1".to_string(), before, after);

        assert_eq!(event.change_type, ChangeType::Update);
        assert!(event.before.is_some());
        assert!(event.after.is_some());
    }

    #[test]
    fn test_change_event_to_event() {
        let source = ChangeSource::new("mydb", "users");
        let data = serde_json::json!({"id": 1});

        let change = ChangeEvent::insert(source, "1".to_string(), data);
        let event = change.to_event();

        assert_eq!(event.event_type, EventType::Created);
        assert_eq!(event.source, "mydb.users");
    }

    #[test]
    fn test_cdc_source_config() {
        let config = CdcSourceConfig::new("mydb", "orders")
            .inserts_only()
            .with_filter("amount > 100");

        assert!(config.should_capture(ChangeType::Insert));
        assert!(!config.should_capture(ChangeType::Update));
        assert!(config.filter.is_some());
    }

    #[test]
    fn test_cdc_config() {
        let config = CdcConfig::new()
            .with_source(CdcSourceConfig::new("db", "table1"))
            .with_batch_size(50);

        assert!(config.enabled);
        assert_eq!(config.sources.len(), 1);
        assert_eq!(config.batch_size, 50);
    }

    #[test]
    fn test_cdc_position() {
        let mut position = CdcPosition::new();
        assert_eq!(position.sequence, 0);

        position.advance();
        assert_eq!(position.sequence, 1);

        position.advance();
        assert_eq!(position.sequence, 2);
    }
}
