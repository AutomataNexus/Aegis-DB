//! Aegis Tracing and Logging
//!
//! Distributed tracing and structured logging for observability.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

// =============================================================================
// Trace ID
// =============================================================================

/// Unique identifier for a trace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(String);

impl TraceId {
    /// Create a new trace ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a new random trace ID.
    pub fn generate() -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self(format!("{:032x}", timestamp))
    }

    /// Get the ID as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TraceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =============================================================================
// Span ID
// =============================================================================

/// Unique identifier for a span within a trace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanId(String);

impl SpanId {
    /// Create a new span ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a new random span ID.
    pub fn generate() -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self(format!("{:016x}", timestamp & 0xFFFFFFFFFFFFFFFF))
    }

    /// Get the ID as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SpanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =============================================================================
// Span Status
// =============================================================================

/// Status of a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    /// Span completed successfully.
    Ok,
    /// Span completed with an error.
    Error,
    /// Span was cancelled.
    Cancelled,
    /// Span is still in progress.
    InProgress,
}

// =============================================================================
// Span Kind
// =============================================================================

/// Kind of span (client, server, internal, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanKind {
    /// Internal operation.
    Internal,
    /// Server receiving a request.
    Server,
    /// Client making a request.
    Client,
    /// Producer sending a message.
    Producer,
    /// Consumer receiving a message.
    Consumer,
}

impl Default for SpanKind {
    fn default() -> Self {
        Self::Internal
    }
}

// =============================================================================
// Span Event
// =============================================================================

/// An event within a span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp: u64,
    pub attributes: HashMap<String, String>,
}

impl SpanEvent {
    /// Create a new span event.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            timestamp: Self::now(),
            attributes: HashMap::new(),
        }
    }

    /// Add an attribute.
    pub fn with_attribute(mut self, key: &str, value: &str) -> Self {
        self.attributes.insert(key.to_string(), value.to_string());
        self
    }

    /// Get current timestamp.
    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
}

// =============================================================================
// Span
// =============================================================================

/// A span representing a unit of work in a trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub name: String,
    pub kind: SpanKind,
    pub status: SpanStatus,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub attributes: HashMap<String, String>,
    pub events: Vec<SpanEvent>,
}

impl Span {
    /// Create a new span.
    pub fn new(trace_id: TraceId, name: &str) -> Self {
        Self {
            trace_id,
            span_id: SpanId::generate(),
            parent_span_id: None,
            name: name.to_string(),
            kind: SpanKind::Internal,
            status: SpanStatus::InProgress,
            start_time: Self::now(),
            end_time: None,
            attributes: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Create a child span.
    pub fn child(&self, name: &str) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: SpanId::generate(),
            parent_span_id: Some(self.span_id.clone()),
            name: name.to_string(),
            kind: SpanKind::Internal,
            status: SpanStatus::InProgress,
            start_time: Self::now(),
            end_time: None,
            attributes: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Set the span kind.
    pub fn with_kind(mut self, kind: SpanKind) -> Self {
        self.kind = kind;
        self
    }

    /// Add an attribute.
    pub fn set_attribute(&mut self, key: &str, value: &str) {
        self.attributes.insert(key.to_string(), value.to_string());
    }

    /// Add an event.
    pub fn add_event(&mut self, event: SpanEvent) {
        self.events.push(event);
    }

    /// Record an event by name.
    pub fn record_event(&mut self, name: &str) {
        self.events.push(SpanEvent::new(name));
    }

    /// Record an exception.
    pub fn record_exception(&mut self, error: &str) {
        let event = SpanEvent::new("exception").with_attribute("message", error);
        self.events.push(event);
        self.status = SpanStatus::Error;
    }

    /// End the span successfully.
    pub fn end(&mut self) {
        self.end_time = Some(Self::now());
        if self.status == SpanStatus::InProgress {
            self.status = SpanStatus::Ok;
        }
    }

    /// End the span with an error.
    pub fn end_with_error(&mut self, error: &str) {
        self.record_exception(error);
        self.end_time = Some(Self::now());
        self.status = SpanStatus::Error;
    }

    /// Get the duration of the span.
    pub fn duration(&self) -> Option<Duration> {
        self.end_time
            .map(|end| Duration::from_nanos(end - self.start_time))
    }

    /// Check if the span is finished.
    pub fn is_finished(&self) -> bool {
        self.end_time.is_some()
    }

    /// Get current timestamp in nanoseconds.
    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
}

// =============================================================================
// Span Builder
// =============================================================================

/// Builder for creating spans.
pub struct SpanBuilder {
    trace_id: TraceId,
    parent_span_id: Option<SpanId>,
    name: String,
    kind: SpanKind,
    attributes: HashMap<String, String>,
}

impl SpanBuilder {
    /// Create a new span builder.
    pub fn new(name: &str) -> Self {
        Self {
            trace_id: TraceId::generate(),
            parent_span_id: None,
            name: name.to_string(),
            kind: SpanKind::Internal,
            attributes: HashMap::new(),
        }
    }

    /// Set the trace ID.
    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = trace_id;
        self
    }

    /// Set the parent span ID.
    pub fn with_parent(mut self, parent_span_id: SpanId) -> Self {
        self.parent_span_id = Some(parent_span_id);
        self
    }

    /// Set the span kind.
    pub fn with_kind(mut self, kind: SpanKind) -> Self {
        self.kind = kind;
        self
    }

    /// Add an attribute.
    pub fn with_attribute(mut self, key: &str, value: &str) -> Self {
        self.attributes.insert(key.to_string(), value.to_string());
        self
    }

    /// Build the span.
    pub fn build(self) -> Span {
        Span {
            trace_id: self.trace_id,
            span_id: SpanId::generate(),
            parent_span_id: self.parent_span_id,
            name: self.name,
            kind: self.kind,
            status: SpanStatus::InProgress,
            start_time: Span::now(),
            end_time: None,
            attributes: self.attributes,
            events: Vec::new(),
        }
    }
}

// =============================================================================
// Trace Context
// =============================================================================

/// Context for propagating trace information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub sampled: bool,
}

impl TraceContext {
    /// Create a new trace context.
    pub fn new(trace_id: TraceId, span_id: SpanId) -> Self {
        Self {
            trace_id,
            span_id,
            sampled: true,
        }
    }

    /// Create from a span.
    pub fn from_span(span: &Span) -> Self {
        Self {
            trace_id: span.trace_id.clone(),
            span_id: span.span_id.clone(),
            sampled: true,
        }
    }

    /// Encode to W3C Trace Context format.
    pub fn to_traceparent(&self) -> String {
        format!(
            "00-{}-{}-{:02x}",
            self.trace_id.as_str(),
            self.span_id.as_str(),
            if self.sampled { 1 } else { 0 }
        )
    }

    /// Parse from W3C Trace Context format.
    pub fn from_traceparent(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() != 4 || parts[0] != "00" {
            return None;
        }

        let sampled = u8::from_str_radix(parts[3], 16).ok()? & 1 == 1;

        Some(Self {
            trace_id: TraceId::new(parts[1]),
            span_id: SpanId::new(parts[2]),
            sampled,
        })
    }
}

// =============================================================================
// Tracer
// =============================================================================

/// A tracer for creating and managing spans.
pub struct Tracer {
    service_name: String,
    spans: RwLock<Vec<Span>>,
    current_span: RwLock<Option<SpanId>>,
}

impl Tracer {
    /// Create a new tracer.
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
            spans: RwLock::new(Vec::new()),
            current_span: RwLock::new(None),
        }
    }

    /// Start a new trace.
    pub fn start_trace(&self, name: &str) -> Span {
        let mut span = Span::new(TraceId::generate(), name);
        span.set_attribute("service.name", &self.service_name);
        *self.current_span.write().unwrap() = Some(span.span_id.clone());
        span
    }

    /// Start a span with a specific trace context.
    pub fn start_span_with_context(&self, name: &str, context: &TraceContext) -> Span {
        let mut span = SpanBuilder::new(name)
            .with_trace_id(context.trace_id.clone())
            .with_parent(context.span_id.clone())
            .build();
        span.set_attribute("service.name", &self.service_name);
        *self.current_span.write().unwrap() = Some(span.span_id.clone());
        span
    }

    /// Start a child span from the current span.
    pub fn start_child(&self, name: &str, parent: &Span) -> Span {
        let mut span = parent.child(name);
        span.set_attribute("service.name", &self.service_name);
        span
    }

    /// Record a completed span.
    pub fn record_span(&self, span: Span) {
        self.spans.write().unwrap().push(span);
    }

    /// Get all recorded spans.
    pub fn get_spans(&self) -> Vec<Span> {
        self.spans.read().unwrap().clone()
    }

    /// Clear all recorded spans.
    pub fn clear_spans(&self) {
        self.spans.write().unwrap().clear();
    }

    /// Get the current span ID.
    pub fn current_span_id(&self) -> Option<SpanId> {
        self.current_span.read().unwrap().clone()
    }
}

// =============================================================================
// Log Level
// =============================================================================

/// Log level for structured logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Convert to string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

// =============================================================================
// Log Entry
// =============================================================================

/// A structured log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: u64,
    pub level: LogLevel,
    pub message: String,
    pub target: String,
    pub trace_id: Option<TraceId>,
    pub span_id: Option<SpanId>,
    pub fields: HashMap<String, String>,
}

impl LogEntry {
    /// Create a new log entry.
    pub fn new(level: LogLevel, message: &str, target: &str) -> Self {
        Self {
            timestamp: Self::now(),
            level,
            message: message.to_string(),
            target: target.to_string(),
            trace_id: None,
            span_id: None,
            fields: HashMap::new(),
        }
    }

    /// Add a field.
    pub fn with_field(mut self, key: &str, value: &str) -> Self {
        self.fields.insert(key.to_string(), value.to_string());
        self
    }

    /// Add trace context.
    pub fn with_trace_context(mut self, context: &TraceContext) -> Self {
        self.trace_id = Some(context.trace_id.clone());
        self.span_id = Some(context.span_id.clone());
        self
    }

    /// Format as JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Get current timestamp.
    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

// =============================================================================
// Logger
// =============================================================================

/// Structured logger with trace context support.
pub struct Logger {
    target: String,
    min_level: LogLevel,
    entries: RwLock<Vec<LogEntry>>,
    context: RwLock<Option<TraceContext>>,
}

impl Logger {
    /// Create a new logger.
    pub fn new(target: &str) -> Self {
        Self {
            target: target.to_string(),
            min_level: LogLevel::Info,
            entries: RwLock::new(Vec::new()),
            context: RwLock::new(None),
        }
    }

    /// Set the minimum log level.
    pub fn with_min_level(mut self, level: LogLevel) -> Self {
        self.min_level = level;
        self
    }

    /// Set the trace context.
    pub fn set_context(&self, context: TraceContext) {
        *self.context.write().unwrap() = Some(context);
    }

    /// Clear the trace context.
    pub fn clear_context(&self) {
        *self.context.write().unwrap() = None;
    }

    /// Log a message.
    pub fn log(&self, level: LogLevel, message: &str) {
        if level < self.min_level {
            return;
        }

        let mut entry = LogEntry::new(level, message, &self.target);

        if let Some(ref ctx) = *self.context.read().unwrap() {
            entry = entry.with_trace_context(ctx);
        }

        self.entries.write().unwrap().push(entry);
    }

    /// Log with fields.
    pub fn log_with_fields(&self, level: LogLevel, message: &str, fields: HashMap<String, String>) {
        if level < self.min_level {
            return;
        }

        let mut entry = LogEntry::new(level, message, &self.target);
        entry.fields = fields;

        if let Some(ref ctx) = *self.context.read().unwrap() {
            entry = entry.with_trace_context(ctx);
        }

        self.entries.write().unwrap().push(entry);
    }

    /// Log at trace level.
    pub fn trace(&self, message: &str) {
        self.log(LogLevel::Trace, message);
    }

    /// Log at debug level.
    pub fn debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }

    /// Log at info level.
    pub fn info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }

    /// Log at warn level.
    pub fn warn(&self, message: &str) {
        self.log(LogLevel::Warn, message);
    }

    /// Log at error level.
    pub fn error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }

    /// Get all log entries.
    pub fn get_entries(&self) -> Vec<LogEntry> {
        self.entries.read().unwrap().clone()
    }

    /// Clear all log entries.
    pub fn clear(&self) {
        self.entries.write().unwrap().clear();
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_id() {
        let id1 = TraceId::generate();
        let id2 = TraceId::generate();

        assert_ne!(id1, id2);
        assert!(!id1.as_str().is_empty());
    }

    #[test]
    fn test_span_id() {
        let id1 = SpanId::generate();
        let id2 = SpanId::generate();

        assert_ne!(id1, id2);
    }

    #[test]
    fn test_span_creation() {
        let trace_id = TraceId::generate();
        let span = Span::new(trace_id.clone(), "test_span");

        assert_eq!(span.trace_id, trace_id);
        assert_eq!(span.name, "test_span");
        assert_eq!(span.status, SpanStatus::InProgress);
        assert!(span.parent_span_id.is_none());
    }

    #[test]
    fn test_span_child() {
        let trace_id = TraceId::generate();
        let parent = Span::new(trace_id.clone(), "parent");
        let child = parent.child("child");

        assert_eq!(child.trace_id, parent.trace_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id.clone()));
    }

    #[test]
    fn test_span_end() {
        let trace_id = TraceId::generate();
        let mut span = Span::new(trace_id, "test");

        assert!(!span.is_finished());

        span.end();

        assert!(span.is_finished());
        assert_eq!(span.status, SpanStatus::Ok);
        assert!(span.duration().is_some());
    }

    #[test]
    fn test_span_error() {
        let trace_id = TraceId::generate();
        let mut span = Span::new(trace_id, "test");

        span.end_with_error("something went wrong");

        assert_eq!(span.status, SpanStatus::Error);
        assert!(!span.events.is_empty());
    }

    #[test]
    fn test_span_builder() {
        let span = SpanBuilder::new("test")
            .with_kind(SpanKind::Server)
            .with_attribute("http.method", "GET")
            .build();

        assert_eq!(span.name, "test");
        assert_eq!(span.kind, SpanKind::Server);
        assert_eq!(span.attributes.get("http.method"), Some(&"GET".to_string()));
    }

    #[test]
    fn test_trace_context() {
        let ctx = TraceContext::new(TraceId::generate(), SpanId::generate());

        let traceparent = ctx.to_traceparent();
        assert!(traceparent.starts_with("00-"));

        let parsed = TraceContext::from_traceparent(&traceparent).unwrap();
        assert_eq!(parsed.trace_id, ctx.trace_id);
        assert_eq!(parsed.span_id, ctx.span_id);
    }

    #[test]
    fn test_tracer() {
        let tracer = Tracer::new("test-service");

        let mut span = tracer.start_trace("root");
        span.set_attribute("key", "value");
        span.end();

        tracer.record_span(span.clone());

        let spans = tracer.get_spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].attributes.get("service.name"), Some(&"test-service".to_string()));
    }

    #[test]
    fn test_log_entry() {
        let entry = LogEntry::new(LogLevel::Info, "test message", "test")
            .with_field("key", "value");

        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.message, "test message");
        assert_eq!(entry.fields.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_logger() {
        let logger = Logger::new("test").with_min_level(LogLevel::Debug);

        logger.trace("should be filtered");
        logger.debug("debug message");
        logger.info("info message");
        logger.error("error message");

        let entries = logger.get_entries();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_logger_with_context() {
        let logger = Logger::new("test");
        let ctx = TraceContext::new(TraceId::generate(), SpanId::generate());

        logger.set_context(ctx.clone());
        logger.info("test message");

        let entries = logger.get_entries();
        assert_eq!(entries[0].trace_id, Some(ctx.trace_id));
    }

    #[test]
    fn test_span_event() {
        let event = SpanEvent::new("db.query")
            .with_attribute("db.statement", "SELECT * FROM users");

        assert_eq!(event.name, "db.query");
        assert_eq!(
            event.attributes.get("db.statement"),
            Some(&"SELECT * FROM users".to_string())
        );
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }
}
