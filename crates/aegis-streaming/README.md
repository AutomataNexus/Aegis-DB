<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/assets/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-streaming

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.0-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Real-time streaming engine for the Aegis Database Platform.

## Overview

`aegis-streaming` provides pub/sub messaging, change data capture (CDC), event sourcing, and stream processing capabilities. It enables real-time data flows and reactive applications.

## Features

- **Pub/Sub Messaging** - Topic-based message publishing and subscription
- **Change Data Capture** - Track all data changes in real-time
- **Event Sourcing** - Store events as the source of truth
- **Stream Processing** - Transform and aggregate streaming data
- **Persistent Subscriptions** - Durable message delivery

## Architecture

```
┌─────────────────────────────────────────────────┐
│              Streaming Engine                    │
├─────────────────────────────────────────────────┤
│              Channel Manager                     │
│  ┌──────────┬──────────────┬─────────────────┐  │
│  │  Topics  │  Partitions  │  Consumer       │  │
│  │          │              │  Groups         │  │
│  └──────────┴──────────────┴─────────────────┘  │
├─────────────────────────────────────────────────┤
│              CDC Engine                          │
│  ┌──────────┬──────────────┬─────────────────┐  │
│  │  WAL     │   Change     │  Subscription   │  │
│  │  Reader  │   Tracker    │   Manager       │  │
│  └──────────┴──────────────┴─────────────────┘  │
├─────────────────────────────────────────────────┤
│           Event Store (Append-Only)             │
└─────────────────────────────────────────────────┘
```

## Modules

| Module | Description |
|--------|-------------|
| `engine` | Main streaming engine |
| `channel` | Pub/sub channel management |
| `stream` | Stream abstraction |
| `subscriber` | Subscription handling |
| `cdc` | Change data capture |
| `event` | Event definitions |

## Usage

```toml
[dependencies]
aegis-streaming = { path = "../aegis-streaming" }
```

### Pub/Sub Messaging

```rust
use aegis_streaming::{StreamingEngine, Topic, Message};

let engine = StreamingEngine::new(config)?;

// Create a topic
engine.create_topic("orders", TopicConfig {
    partitions: 4,
    retention: Duration::from_days(7),
    replication_factor: 3,
})?;

// Publish messages
engine.publish("orders", Message {
    key: Some("order-123".into()),
    payload: json!({"id": "123", "amount": 99.99}),
    headers: HashMap::new(),
}).await?;

// Subscribe to messages
let mut subscriber = engine.subscribe("orders", SubscribeConfig {
    group_id: "order-processor",
    from: Offset::Latest,
}).await?;

while let Some(msg) = subscriber.next().await {
    println!("Received: {:?}", msg.payload);
    msg.ack().await?;
}
```

### Change Data Capture (CDC)

```rust
use aegis_streaming::cdc::{CDCEngine, ChangeEvent};

let cdc = CDCEngine::new(storage)?;

// Subscribe to changes on a table
let mut changes = cdc.subscribe("users", CDCConfig {
    operations: vec![Operation::Insert, Operation::Update, Operation::Delete],
    include_old_values: true,
}).await?;

while let Some(event) = changes.next().await {
    match event {
        ChangeEvent::Insert { new_row, .. } => {
            println!("New user: {:?}", new_row);
        }
        ChangeEvent::Update { old_row, new_row, .. } => {
            println!("Updated from {:?} to {:?}", old_row, new_row);
        }
        ChangeEvent::Delete { old_row, .. } => {
            println!("Deleted: {:?}", old_row);
        }
    }
}
```

### Event Sourcing

```rust
use aegis_streaming::event::{EventStore, Event, AggregateRoot};

let store = EventStore::new(storage)?;

// Append events
store.append("account-123", vec![
    Event::new("AccountCreated", json!({"owner": "Alice"})),
    Event::new("MoneyDeposited", json!({"amount": 100.0})),
    Event::new("MoneyWithdrawn", json!({"amount": 30.0})),
]).await?;

// Load aggregate from events
let events = store.load("account-123").await?;
let account = Account::from_events(events);
println!("Balance: {}", account.balance);

// Subscribe to events
let mut stream = store.subscribe("account-*").await?;
while let Some(event) = stream.next().await {
    println!("Event: {} on {}", event.event_type, event.aggregate_id);
}
```

### Stream Processing

```rust
use aegis_streaming::stream::{Stream, StreamProcessor};

let processor = StreamProcessor::new();

// Define processing pipeline
processor
    .source("raw-events")
    .filter(|e| e["type"] == "purchase")
    .map(|e| json!({
        "customer": e["customer_id"],
        "amount": e["total"],
        "timestamp": e["created_at"]
    }))
    .window(Duration::from_mins(5))
    .aggregate(|window| json!({
        "total_purchases": window.len(),
        "total_amount": window.iter().map(|e| e["amount"].as_f64().unwrap()).sum::<f64>()
    }))
    .sink("purchase-metrics")
    .start()
    .await?;
```

### Consumer Groups

```rust
// Multiple consumers in a group share the load
let consumer1 = engine.subscribe("orders", SubscribeConfig {
    group_id: "processors",
    ..Default::default()
}).await?;

let consumer2 = engine.subscribe("orders", SubscribeConfig {
    group_id: "processors",  // Same group
    ..Default::default()
}).await?;

// Each message is delivered to only one consumer in the group
```

## Message Delivery Guarantees

| Mode | Description |
|------|-------------|
| `AtMostOnce` | Fire and forget, may lose messages |
| `AtLeastOnce` | Guaranteed delivery, may duplicate |
| `ExactlyOnce` | Guaranteed exactly-once (with transactions) |

## Configuration

```toml
[streaming]
max_message_size = "1MB"
default_retention = "7d"

[streaming.consumer]
max_poll_records = 500
auto_commit = false
session_timeout = "30s"

[streaming.cdc]
enabled = true
buffer_size = 10000
```

## Tests

```bash
cargo test -p aegis-streaming
```

**Test count:** 31 tests

## License

Apache-2.0
