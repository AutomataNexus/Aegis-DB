# aegis-streaming

Real-time Streaming Engine for Aegis Database Platform.

## Overview

Event streaming and change data capture for real-time data processing.
Supports pub/sub messaging, event sourcing, and stream processing.

## Modules

### event.rs
Core event types:
- `EventId` - Unique event identifier
- `EventType` - Created, Updated, Deleted, Custom
- `Event` - Event with source, timestamp, data, metadata
- `EventData` - Payload (Null, Bool, Int, Float, String, Bytes, Json)
- `EventFilter` - Filter by type, source, timestamp
- `EventBatch` - Batch of events

### channel.rs
Pub/sub channels:
- `ChannelId` - Channel identifier
- `ChannelConfig` - Buffer size, max subscribers, persistence, retention
- `Channel` - Publish/subscribe messaging
- `ChannelReceiver` - Async event receiver with filtering
- `ChannelStats` - Events published, subscriber count
- History support for persistent channels

### subscriber.rs
Subscription management:
- `SubscriberId` - Subscriber identifier
- `Subscription` - Channels, filter, metadata
- `Subscriber` - Tracks subscriptions and statistics
- `DeliveryMode` - AtMostOnce, AtLeastOnce, ExactlyOnce
- `AckMode` - Auto, Manual, None

### cdc.rs
Change data capture:
- `ChangeType` - Insert, Update, Delete, Truncate
- `ChangeEvent` - Before/after data with metadata
- `ChangeSource` - Database, table, schema
- `CdcConfig` - Batch size, timeout, include_before
- `CdcSourceConfig` - Per-table CDC configuration
- `CdcPosition` - Track position for resumable streaming

### stream.rs
Stream processing:
- `EventStream` - Buffered event stream with max size
- `StreamProcessor` - Processing pipeline
- `ProcessingStep` - Filter, Map, TransformData
- `WindowedStream` - Time-windowed events
- `Window` - Time bucket with events
- `AggregateFunction` - Count, Sum, Avg, Min, Max

### engine.rs
Core engine:
- `StreamingEngine` - Main entry point
- Channel management (create, delete, list)
- Publishing (single, batch, to multiple channels)
- Subscribing (with optional filters)
- History retrieval
- Statistics tracking

## Usage Example

```rust
use aegis_streaming::*;

// Create engine
let engine = StreamingEngine::new();

// Create channel
engine.create_channel("events")?;

// Subscribe
let channel_id = ChannelId::new("events");
let mut receiver = engine.subscribe(&channel_id, "subscriber1")?;

// Publish event
let event = Event::new(
    EventType::Created,
    "users",
    EventData::Json(serde_json::json!({"id": 1, "name": "Alice"})),
);
engine.publish(&channel_id, event)?;

// Receive events (async)
let received = receiver.recv().await?;
```

## CDC Example

```rust
use aegis_streaming::*;

// Configure CDC
let config = CdcConfig::new()
    .with_source(CdcSourceConfig::new("mydb", "users"))
    .with_batch_size(100);

// Create change events
let change = ChangeEvent::insert(
    ChangeSource::new("mydb", "users"),
    "123".to_string(),
    serde_json::json!({"id": 123, "name": "Bob"}),
);

// Publish to channel
let event = change.to_event();
engine.publish(&channel_id, event)?;
```

## Stream Processing Example

```rust
use aegis_streaming::*;

// Build processing pipeline
let processor = StreamProcessor::new()
    .filter(EventFilter::new().with_type(EventType::Created))
    .map(|mut e| {
        e.metadata.insert("processed".to_string(), "true".to_string());
        e
    });

// Process events
let processed = processor.process(event);
let batch = processor.process_batch(events);
```

## Tests

31 tests covering all modules.
