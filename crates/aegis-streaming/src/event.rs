//! Aegis Streaming Events
//!
//! Core event types for the streaming system.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// Event ID
// =============================================================================

/// Unique identifier for an event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub String);

impl EventId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn generate() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self(format!("evt_{:032x}", timestamp))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for EventId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for EventId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// =============================================================================
// Event Type
// =============================================================================

/// Type of event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    /// Data was created.
    Created,
    /// Data was updated.
    Updated,
    /// Data was deleted.
    Deleted,
    /// Custom event type.
    Custom(String),
}

impl EventType {
    pub fn custom(name: impl Into<String>) -> Self {
        Self::Custom(name.into())
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Deleted => "deleted",
            Self::Custom(s) => s,
        }
    }
}

impl Default for EventType {
    fn default() -> Self {
        Self::Custom("unknown".to_string())
    }
}

// =============================================================================
// Event
// =============================================================================

/// An event in the streaming system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub event_type: EventType,
    pub source: String,
    pub timestamp: u64,
    pub data: EventData,
    pub metadata: HashMap<String, String>,
}

impl Event {
    /// Create a new event.
    pub fn new(event_type: EventType, source: impl Into<String>, data: EventData) -> Self {
        Self {
            id: EventId::generate(),
            event_type,
            source: source.into(),
            timestamp: current_timestamp_millis(),
            data,
            metadata: HashMap::new(),
        }
    }

    /// Create an event with a specific ID.
    pub fn with_id(
        id: EventId,
        event_type: EventType,
        source: impl Into<String>,
        data: EventData,
    ) -> Self {
        Self {
            id,
            event_type,
            source: source.into(),
            timestamp: current_timestamp_millis(),
            data,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the event.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get metadata value.
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Check if the event matches a filter.
    pub fn matches(&self, filter: &EventFilter) -> bool {
        if let Some(ref event_type) = filter.event_type {
            if &self.event_type != event_type {
                return false;
            }
        }

        if let Some(ref source) = filter.source {
            if !self.source.starts_with(source) {
                return false;
            }
        }

        if let Some(after) = filter.after_timestamp {
            if self.timestamp <= after {
                return false;
            }
        }

        true
    }
}

// =============================================================================
// Event Data
// =============================================================================

/// Data payload for an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[derive(Default)]
pub enum EventData {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Json(serde_json::Value),
}

impl EventData {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Json(v) => Some(v),
            _ => None,
        }
    }
}


impl From<String> for EventData {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for EventData {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<i64> for EventData {
    fn from(n: i64) -> Self {
        Self::Int(n)
    }
}

impl From<f64> for EventData {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<bool> for EventData {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<serde_json::Value> for EventData {
    fn from(v: serde_json::Value) -> Self {
        Self::Json(v)
    }
}

impl From<Vec<u8>> for EventData {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Bytes(bytes)
    }
}

// =============================================================================
// Event Filter
// =============================================================================

/// Filter for events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventFilter {
    pub event_type: Option<EventType>,
    pub source: Option<String>,
    pub after_timestamp: Option<u64>,
}

impl EventFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_type(mut self, event_type: EventType) -> Self {
        self.event_type = Some(event_type);
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn after(mut self, timestamp: u64) -> Self {
        self.after_timestamp = Some(timestamp);
        self
    }
}

// =============================================================================
// Event Batch
// =============================================================================

/// A batch of events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBatch {
    pub events: Vec<Event>,
    pub batch_id: String,
    pub timestamp: u64,
}

impl EventBatch {
    pub fn new(events: Vec<Event>) -> Self {
        let batch_id = format!("batch_{}", current_timestamp_millis());
        Self {
            events,
            batch_id,
            timestamp: current_timestamp_millis(),
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

fn current_timestamp_millis() -> u64 {
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
    fn test_event_id() {
        let id1 = EventId::generate();
        let id2 = EventId::generate();
        assert_ne!(id1, id2);
        assert!(id1.as_str().starts_with("evt_"));
    }

    #[test]
    fn test_event_creation() {
        let event = Event::new(
            EventType::Created,
            "users",
            EventData::String("test data".to_string()),
        );

        assert_eq!(event.event_type, EventType::Created);
        assert_eq!(event.source, "users");
        assert!(event.timestamp > 0);
    }

    #[test]
    fn test_event_metadata() {
        let event = Event::new(EventType::Updated, "orders", EventData::Null)
            .with_metadata("user_id", "123")
            .with_metadata("action", "update");

        assert_eq!(event.get_metadata("user_id"), Some(&"123".to_string()));
        assert_eq!(event.get_metadata("action"), Some(&"update".to_string()));
    }

    #[test]
    fn test_event_filter() {
        let event = Event::new(EventType::Created, "users.profile", EventData::Null);

        let filter = EventFilter::new().with_type(EventType::Created);
        assert!(event.matches(&filter));

        let filter = EventFilter::new().with_source("users");
        assert!(event.matches(&filter));

        let filter = EventFilter::new().with_type(EventType::Deleted);
        assert!(!event.matches(&filter));
    }

    #[test]
    fn test_event_data() {
        let data = EventData::String("hello".to_string());
        assert_eq!(data.as_str(), Some("hello"));

        let data = EventData::Int(42);
        assert_eq!(data.as_i64(), Some(42));

        let data: EventData = "test".into();
        assert_eq!(data.as_str(), Some("test"));
    }
}
