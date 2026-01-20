//! Aegis Streaming Stream Processing
//!
//! Stream processing utilities for event transformation.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::event::{Event, EventData, EventFilter};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::Duration;

// =============================================================================
// Event Stream
// =============================================================================

/// A stream of events with processing capabilities.
pub struct EventStream {
    events: VecDeque<Event>,
    max_size: usize,
}

impl EventStream {
    /// Create a new event stream.
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            max_size: 10_000,
        }
    }

    /// Create a stream with a maximum size.
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            events: VecDeque::new(),
            max_size,
        }
    }

    /// Push an event to the stream.
    pub fn push(&mut self, event: Event) {
        if self.events.len() >= self.max_size {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Pop the next event from the stream.
    pub fn pop(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    /// Peek at the next event.
    pub fn peek(&self) -> Option<&Event> {
        self.events.front()
    }

    /// Get the number of events in the stream.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if the stream is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Clear all events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Filter events in the stream.
    pub fn filter(&self, filter: &EventFilter) -> Vec<&Event> {
        self.events.iter().filter(|e| e.matches(filter)).collect()
    }

    /// Take events matching a filter.
    pub fn take(&mut self, filter: &EventFilter) -> Vec<Event> {
        let matching: Vec<Event> = self
            .events
            .iter()
            .filter(|e| e.matches(filter))
            .cloned()
            .collect();

        self.events.retain(|e| !e.matches(filter));

        matching
    }

    /// Get events as a slice.
    pub fn as_slice(&self) -> impl Iterator<Item = &Event> {
        self.events.iter()
    }
}

impl Default for EventStream {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Stream Processor
// =============================================================================

/// Processes events in a stream.
pub struct StreamProcessor {
    pipeline: Vec<Box<dyn ProcessingStep + Send + Sync>>,
}

impl StreamProcessor {
    /// Create a new stream processor.
    pub fn new() -> Self {
        Self {
            pipeline: Vec::new(),
        }
    }

    /// Add a processing step.
    pub fn add_step(mut self, step: impl ProcessingStep + Send + Sync + 'static) -> Self {
        self.pipeline.push(Box::new(step));
        self
    }

    /// Add a filter step.
    pub fn filter(self, filter: EventFilter) -> Self {
        self.add_step(FilterStep { filter })
    }

    /// Add a map step.
    pub fn map(self, mapper: impl Fn(Event) -> Event + Send + Sync + 'static) -> Self {
        self.add_step(MapStep {
            mapper: Arc::new(mapper),
        })
    }

    /// Add a transform step on event data.
    pub fn transform_data(
        self,
        transformer: impl Fn(EventData) -> EventData + Send + Sync + 'static,
    ) -> Self {
        self.add_step(TransformDataStep {
            transformer: Arc::new(transformer),
        })
    }

    /// Process an event through the pipeline.
    pub fn process(&self, event: Event) -> Option<Event> {
        let mut current = Some(event);

        for step in &self.pipeline {
            if let Some(e) = current {
                current = step.process(e);
            } else {
                break;
            }
        }

        current
    }

    /// Process a batch of events.
    pub fn process_batch(&self, events: Vec<Event>) -> Vec<Event> {
        events.into_iter().filter_map(|e| self.process(e)).collect()
    }
}

impl Default for StreamProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Processing Step
// =============================================================================

/// A step in the processing pipeline.
pub trait ProcessingStep {
    fn process(&self, event: Event) -> Option<Event>;
}

/// Filter step that filters events.
struct FilterStep {
    filter: EventFilter,
}

impl ProcessingStep for FilterStep {
    fn process(&self, event: Event) -> Option<Event> {
        if event.matches(&self.filter) {
            Some(event)
        } else {
            None
        }
    }
}

/// Map step that transforms events.
struct MapStep {
    mapper: Arc<dyn Fn(Event) -> Event + Send + Sync>,
}

impl ProcessingStep for MapStep {
    fn process(&self, event: Event) -> Option<Event> {
        Some((self.mapper)(event))
    }
}

/// Transform step that transforms event data.
struct TransformDataStep {
    transformer: Arc<dyn Fn(EventData) -> EventData + Send + Sync>,
}

impl ProcessingStep for TransformDataStep {
    fn process(&self, mut event: Event) -> Option<Event> {
        event.data = (self.transformer)(event.data);
        Some(event)
    }
}

// =============================================================================
// Windowed Stream
// =============================================================================

/// A time-windowed event stream.
pub struct WindowedStream {
    window_size: Duration,
    windows: RwLock<Vec<Window>>,
}

impl WindowedStream {
    /// Create a windowed stream.
    pub fn new(window_size: Duration) -> Self {
        Self {
            window_size,
            windows: RwLock::new(Vec::new()),
        }
    }

    /// Add an event to the appropriate window.
    pub fn push(&self, event: Event) {
        let window_start = self.window_start(event.timestamp);
        let mut windows = self.windows.write().unwrap();

        if let Some(window) = windows.iter_mut().find(|w| w.start == window_start) {
            window.events.push(event);
        } else {
            let mut window = Window::new(window_start, self.window_size.as_millis() as u64);
            window.events.push(event);
            windows.push(window);
        }
    }

    /// Get completed windows.
    pub fn completed_windows(&self) -> Vec<Window> {
        let now = current_timestamp_millis();
        let windows = self.windows.read().unwrap();

        windows
            .iter()
            .filter(|w| w.end() <= now)
            .cloned()
            .collect()
    }

    /// Remove completed windows and return them.
    pub fn flush_completed(&self) -> Vec<Window> {
        let now = current_timestamp_millis();
        let mut windows = self.windows.write().unwrap();

        let completed: Vec<Window> = windows
            .iter()
            .filter(|w| w.end() <= now)
            .cloned()
            .collect();

        windows.retain(|w| w.end() > now);

        completed
    }

    fn window_start(&self, timestamp: u64) -> u64 {
        let window_ms = self.window_size.as_millis() as u64;
        (timestamp / window_ms) * window_ms
    }
}

/// A time window of events.
#[derive(Debug, Clone)]
pub struct Window {
    pub start: u64,
    pub duration: u64,
    pub events: Vec<Event>,
}

impl Window {
    pub fn new(start: u64, duration: u64) -> Self {
        Self {
            start,
            duration,
            events: Vec::new(),
        }
    }

    pub fn end(&self) -> u64 {
        self.start + self.duration
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

// =============================================================================
// Aggregation
// =============================================================================

/// Aggregate function for stream processing.
#[derive(Debug, Clone, Copy)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

impl AggregateFunction {
    /// Apply the aggregation to numeric values.
    pub fn apply(&self, values: &[f64]) -> Option<f64> {
        if values.is_empty() {
            return None;
        }

        Some(match self {
            Self::Count => values.len() as f64,
            Self::Sum => values.iter().sum(),
            Self::Avg => values.iter().sum::<f64>() / values.len() as f64,
            Self::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
            Self::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        })
    }
}

fn current_timestamp_millis() -> u64 {
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
    use crate::event::EventType;

    fn create_test_event(source: &str, value: i64) -> Event {
        Event::new(EventType::Created, source, EventData::Int(value))
    }

    #[test]
    fn test_event_stream() {
        let mut stream = EventStream::new();

        stream.push(create_test_event("test", 1));
        stream.push(create_test_event("test", 2));
        stream.push(create_test_event("test", 3));

        assert_eq!(stream.len(), 3);

        let event = stream.pop().unwrap();
        assert_eq!(event.data.as_i64(), Some(1));
    }

    #[test]
    fn test_stream_filter() {
        let mut stream = EventStream::new();

        stream.push(create_test_event("users", 1));
        stream.push(create_test_event("orders", 2));
        stream.push(create_test_event("users", 3));

        let filter = EventFilter::new().with_source("users");
        let filtered = stream.filter(&filter);

        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_stream_processor() {
        let processor = StreamProcessor::new()
            .filter(EventFilter::new().with_type(EventType::Created))
            .map(|mut e| {
                e.metadata.insert("processed".to_string(), "true".to_string());
                e
            });

        let event = create_test_event("test", 42);
        let processed = processor.process(event).unwrap();

        assert_eq!(processed.get_metadata("processed"), Some(&"true".to_string()));
    }

    #[test]
    fn test_stream_max_size() {
        let mut stream = EventStream::with_max_size(3);

        for i in 0..5 {
            stream.push(create_test_event("test", i));
        }

        assert_eq!(stream.len(), 3);

        let first = stream.pop().unwrap();
        assert_eq!(first.data.as_i64(), Some(2));
    }

    #[test]
    fn test_aggregate_functions() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        assert_eq!(AggregateFunction::Count.apply(&values), Some(5.0));
        assert_eq!(AggregateFunction::Sum.apply(&values), Some(15.0));
        assert_eq!(AggregateFunction::Avg.apply(&values), Some(3.0));
        assert_eq!(AggregateFunction::Min.apply(&values), Some(1.0));
        assert_eq!(AggregateFunction::Max.apply(&values), Some(5.0));
    }

    #[test]
    fn test_window() {
        let window = Window::new(1000, 100);
        assert_eq!(window.start, 1000);
        assert_eq!(window.end(), 1100);
        assert!(window.is_empty());
    }
}
