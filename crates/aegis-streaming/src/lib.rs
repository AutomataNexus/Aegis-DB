//! Aegis Streaming - Real-Time Event Processing
//!
//! Event streaming and change data capture for real-time data processing.
//! Supports pub/sub messaging, event sourcing, and CQRS patterns.
//!
//! Key Features:
//! - Change data capture (CDC) with configurable triggers
//! - WebSocket and Server-Sent Events support
//! - Pub/sub messaging with persistent subscriptions
//! - Event sourcing and CQRS support
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod event;
pub mod channel;
pub mod subscriber;
pub mod cdc;
pub mod stream;
pub mod engine;

pub use event::{Event, EventId, EventType};
pub use channel::{Channel, ChannelId};
pub use subscriber::{Subscriber, SubscriberId, Subscription};
pub use cdc::{ChangeEvent, ChangeType, CdcConfig};
pub use stream::{EventStream, StreamProcessor};
pub use engine::StreamingEngine;
