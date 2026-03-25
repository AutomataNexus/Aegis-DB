//! Aegis Streaming Channels
//!
//! Pub/sub channels for event distribution.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::event::{Event, EventFilter};
use crate::subscriber::{AckMode, SubscriberId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

// =============================================================================
// Channel ID
// =============================================================================

/// Unique identifier for a channel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub String);

impl ChannelId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ChannelId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ChannelId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// =============================================================================
// Channel Configuration
// =============================================================================

/// Configuration for a channel.
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub buffer_size: usize,
    pub max_subscribers: usize,
    pub persistent: bool,
    pub retention_count: usize,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1024,
            max_subscribers: 1000,
            persistent: false,
            retention_count: 1000,
        }
    }
}

// =============================================================================
// Channel
// =============================================================================

/// A pub/sub channel for event distribution.
pub struct Channel {
    id: ChannelId,
    config: ChannelConfig,
    sender: broadcast::Sender<Event>,
    subscribers: RwLock<HashMap<SubscriberId, SubscriberInfo>>,
    history: RwLock<VecDeque<Event>>,
    stats: RwLock<ChannelStats>,
}

impl Channel {
    /// Create a new channel.
    pub fn new(id: impl Into<ChannelId>) -> Self {
        Self::with_config(id, ChannelConfig::default())
    }

    /// Create a channel with custom configuration.
    pub fn with_config(id: impl Into<ChannelId>, config: ChannelConfig) -> Self {
        let (sender, _) = broadcast::channel(config.buffer_size);

        Self {
            id: id.into(),
            config,
            sender,
            subscribers: RwLock::new(HashMap::new()),
            history: RwLock::new(VecDeque::new()),
            stats: RwLock::new(ChannelStats::default()),
        }
    }

    /// Get the channel ID.
    pub fn id(&self) -> &ChannelId {
        &self.id
    }

    /// Publish an event to the channel.
    pub fn publish(&self, event: Event) -> Result<usize, ChannelError> {
        if self.config.persistent {
            let mut history = self
                .history
                .write()
                .expect("history RwLock poisoned in publish");
            history.push_back(event.clone());

            while history.len() > self.config.retention_count {
                history.pop_front();
            }
        }

        let receivers = self.sender.send(event).unwrap_or(0);

        {
            let mut stats = self
                .stats
                .write()
                .expect("stats RwLock poisoned in publish");
            stats.events_published += 1;
            stats.last_event_time = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
            );
        }

        Ok(receivers)
    }

    /// Subscribe to the channel.
    pub fn subscribe(&self, subscriber_id: SubscriberId) -> Result<ChannelReceiver, ChannelError> {
        self.subscribe_with_ack_mode(subscriber_id, None, AckMode::Auto)
    }

    /// Subscribe with a filter.
    pub fn subscribe_with_filter(
        &self,
        subscriber_id: SubscriberId,
        filter: EventFilter,
    ) -> Result<ChannelReceiver, ChannelError> {
        self.subscribe_with_ack_mode(subscriber_id, Some(filter), AckMode::Auto)
    }

    /// Subscribe with a specific ack mode.
    pub fn subscribe_with_ack_mode(
        &self,
        subscriber_id: SubscriberId,
        filter: Option<EventFilter>,
        ack_mode: AckMode,
    ) -> Result<ChannelReceiver, ChannelError> {
        let subscribers = self
            .subscribers
            .read()
            .expect("subscribers RwLock poisoned in subscribe_with_ack_mode (read)");
        if subscribers.len() >= self.config.max_subscribers {
            return Err(ChannelError::TooManySubscribers);
        }
        drop(subscribers);

        let receiver = self.sender.subscribe();

        {
            let mut subscribers = self
                .subscribers
                .write()
                .expect("subscribers RwLock poisoned in subscribe_with_ack_mode (write)");
            subscribers.insert(
                subscriber_id.clone(),
                SubscriberInfo {
                    filter: filter.clone(),
                    subscribed_at: current_timestamp(),
                },
            );
        }

        {
            let mut stats = self
                .stats
                .write()
                .expect("stats RwLock poisoned in subscribe_with_ack_mode");
            stats.subscriber_count += 1;
        }

        Ok(ChannelReceiver {
            receiver,
            filter,
            ack_mode,
            next_offset: 0,
            ack_state: Arc::new(RwLock::new(AckState {
                unacked: HashMap::new(),
                acked: HashSet::new(),
            })),
        })
    }

    /// Unsubscribe from the channel.
    pub fn unsubscribe(&self, subscriber_id: &SubscriberId) {
        let mut subscribers = self
            .subscribers
            .write()
            .expect("subscribers RwLock poisoned in unsubscribe");
        if subscribers.remove(subscriber_id).is_some() {
            let mut stats = self
                .stats
                .write()
                .expect("stats RwLock poisoned in unsubscribe");
            stats.subscriber_count = stats.subscriber_count.saturating_sub(1);
        }
    }

    /// Get the number of subscribers.
    pub fn subscriber_count(&self) -> usize {
        let subscribers = self
            .subscribers
            .read()
            .expect("subscribers RwLock poisoned in subscriber_count");
        subscribers.len()
    }

    /// Get recent events from history.
    pub fn get_history(&self, count: usize) -> Vec<Event> {
        let history = self
            .history
            .read()
            .expect("history RwLock poisoned in get_history");
        history.iter().rev().take(count).cloned().collect()
    }

    /// Get events from history after a timestamp.
    pub fn get_history_after(&self, timestamp: u64) -> Vec<Event> {
        let history = self
            .history
            .read()
            .expect("history RwLock poisoned in get_history_after");
        history
            .iter()
            .filter(|e| e.timestamp > timestamp)
            .cloned()
            .collect()
    }

    /// Get channel statistics.
    pub fn stats(&self) -> ChannelStats {
        let stats = self.stats.read().expect("stats RwLock poisoned in stats");
        stats.clone()
    }

    /// Clear history.
    pub fn clear_history(&self) {
        let mut history = self
            .history
            .write()
            .expect("history RwLock poisoned in clear_history");
        history.clear();
    }
}

// =============================================================================
// Channel Receiver
// =============================================================================

/// Shared state for tracking unacknowledged messages across clones.
#[derive(Debug)]
struct AckState {
    /// Messages that have been delivered but not yet acknowledged, keyed by offset.
    unacked: HashMap<u64, Event>,
    /// The set of offsets that have been acknowledged.
    acked: HashSet<u64>,
}

/// Receiver for channel events.
pub struct ChannelReceiver {
    receiver: broadcast::Receiver<Event>,
    filter: Option<EventFilter>,
    ack_mode: AckMode,
    /// Monotonically increasing offset assigned to each received message.
    next_offset: u64,
    /// Shared ack tracking state (used when ack_mode is Manual).
    ack_state: Arc<RwLock<AckState>>,
}

impl ChannelReceiver {
    /// Get the current ack mode for this receiver.
    pub fn ack_mode(&self) -> AckMode {
        self.ack_mode
    }

    /// Get the next offset that will be assigned (i.e., total messages received so far).
    pub fn current_offset(&self) -> u64 {
        self.next_offset
    }

    /// Receive the next event.
    ///
    /// When `AckMode::Auto`, messages are automatically acknowledged on receive.
    /// When `AckMode::Manual`, messages are tracked as unacknowledged and must be
    /// explicitly acknowledged via [`ack`]. When `AckMode::None`, no tracking is performed.
    pub async fn recv(&mut self) -> Result<Event, ChannelError> {
        loop {
            match self.receiver.recv().await {
                Ok(event) => {
                    if let Some(ref filter) = self.filter {
                        if !event.matches(filter) {
                            continue;
                        }
                    }
                    let offset = self.next_offset;
                    self.next_offset += 1;

                    match self.ack_mode {
                        AckMode::Auto => {
                            // Auto-ack: record as already acknowledged
                            let mut state = self
                                .ack_state
                                .write()
                                .expect("ack_state RwLock poisoned in recv");
                            state.acked.insert(offset);
                        }
                        AckMode::Manual => {
                            // Track as unacked for manual acknowledgment
                            let mut state = self
                                .ack_state
                                .write()
                                .expect("ack_state RwLock poisoned in recv");
                            state.unacked.insert(offset, event.clone());
                        }
                        AckMode::None => {
                            // No tracking
                        }
                    }

                    return Ok(event);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(ChannelError::Closed);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    return Err(ChannelError::Lagged(n));
                }
            }
        }
    }

    /// Try to receive an event without blocking.
    pub fn try_recv(&mut self) -> Result<Option<Event>, ChannelError> {
        loop {
            match self.receiver.try_recv() {
                Ok(event) => {
                    if let Some(ref filter) = self.filter {
                        if !event.matches(filter) {
                            continue;
                        }
                    }
                    let offset = self.next_offset;
                    self.next_offset += 1;

                    match self.ack_mode {
                        AckMode::Auto => {
                            let mut state = self
                                .ack_state
                                .write()
                                .expect("ack_state RwLock poisoned in try_recv");
                            state.acked.insert(offset);
                        }
                        AckMode::Manual => {
                            let mut state = self
                                .ack_state
                                .write()
                                .expect("ack_state RwLock poisoned in try_recv");
                            state.unacked.insert(offset, event.clone());
                        }
                        AckMode::None => {}
                    }

                    return Ok(Some(event));
                }
                Err(broadcast::error::TryRecvError::Empty) => {
                    return Ok(None);
                }
                Err(broadcast::error::TryRecvError::Closed) => {
                    return Err(ChannelError::Closed);
                }
                Err(broadcast::error::TryRecvError::Lagged(n)) => {
                    return Err(ChannelError::Lagged(n));
                }
            }
        }
    }

    /// Acknowledge a message by its offset.
    ///
    /// Only meaningful when `AckMode::Manual`. Removes the message from the
    /// unacknowledged set. Returns an error if the offset is not found.
    pub fn ack(&self, offset: u64) -> Result<(), ChannelError> {
        let mut state = self
            .ack_state
            .write()
            .expect("ack_state RwLock poisoned in ack");
        if state.unacked.remove(&offset).is_some() {
            state.acked.insert(offset);
            Ok(())
        } else if state.acked.contains(&offset) {
            // Already acked, idempotent
            Ok(())
        } else {
            Err(ChannelError::NotFound(offset))
        }
    }

    /// Get the number of unacknowledged messages.
    pub fn unacked_count(&self) -> usize {
        let state = self
            .ack_state
            .read()
            .expect("ack_state RwLock poisoned in unacked_count");
        state.unacked.len()
    }

    /// Get the number of acknowledged messages.
    pub fn acked_count(&self) -> usize {
        let state = self
            .ack_state
            .read()
            .expect("ack_state RwLock poisoned in acked_count");
        state.acked.len()
    }

    /// Get all unacknowledged messages for re-delivery.
    ///
    /// Returns a vector of (offset, event) pairs for messages that have been
    /// received but not yet acknowledged.
    pub fn get_unacked_messages(&self) -> Vec<(u64, Event)> {
        let state = self
            .ack_state
            .read()
            .expect("ack_state RwLock poisoned in get_unacked_messages");
        let mut messages: Vec<(u64, Event)> = state
            .unacked
            .iter()
            .map(|(offset, event)| (*offset, event.clone()))
            .collect();
        messages.sort_by_key(|(offset, _)| *offset);
        messages
    }
}

// =============================================================================
// Subscriber Info
// =============================================================================

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SubscriberInfo {
    filter: Option<EventFilter>,
    subscribed_at: u64,
}

// =============================================================================
// Channel Statistics
// =============================================================================

/// Statistics for a channel.
#[derive(Debug, Clone, Default)]
pub struct ChannelStats {
    pub events_published: u64,
    pub subscriber_count: usize,
    pub last_event_time: Option<u64>,
}

// =============================================================================
// Channel Error
// =============================================================================

/// Errors that can occur with channels.
#[derive(Debug, Clone)]
pub enum ChannelError {
    TooManySubscribers,
    Closed,
    Lagged(u64),
    SendFailed,
    /// The message at the given offset was not found in unacked messages.
    NotFound(u64),
}

impl std::fmt::Display for ChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManySubscribers => write!(f, "Maximum subscribers reached"),
            Self::Closed => write!(f, "Channel is closed"),
            Self::Lagged(n) => write!(f, "Receiver lagged by {} messages", n),
            Self::SendFailed => write!(f, "Failed to send event"),
            Self::NotFound(offset) => {
                write!(f, "Message at offset {} not found in unacked set", offset)
            }
        }
    }
}

impl std::error::Error for ChannelError {}

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
    use crate::event::EventData;

    #[test]
    fn test_channel_creation() {
        let channel = Channel::new("test");
        assert_eq!(channel.id().as_str(), "test");
        assert_eq!(channel.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_publish_subscribe() {
        let channel = Channel::new("events");
        let sub_id = SubscriberId::new("sub1");

        let mut receiver = channel.subscribe(sub_id).unwrap();

        let event = Event::new(
            crate::event::EventType::Created,
            "test",
            EventData::String("hello".to_string()),
        );

        channel.publish(event.clone()).unwrap();

        let received = receiver.recv().await.unwrap();
        assert_eq!(received.source, "test");
    }

    #[test]
    fn test_channel_history() {
        let config = ChannelConfig {
            persistent: true,
            retention_count: 10,
            ..Default::default()
        };
        let channel = Channel::with_config("history_test", config);

        for i in 0..5 {
            let event = Event::new(crate::event::EventType::Created, "test", EventData::Int(i));
            channel.publish(event).unwrap();
        }

        let history = channel.get_history(10);
        assert_eq!(history.len(), 5);
    }

    #[test]
    fn test_subscriber_limit() {
        let config = ChannelConfig {
            max_subscribers: 2,
            ..Default::default()
        };
        let channel = Channel::with_config("limited", config);

        channel.subscribe(SubscriberId::new("sub1")).unwrap();
        channel.subscribe(SubscriberId::new("sub2")).unwrap();

        let result = channel.subscribe(SubscriberId::new("sub3"));
        assert!(matches!(result, Err(ChannelError::TooManySubscribers)));
    }

    #[tokio::test]
    async fn test_auto_ack_mode() {
        use crate::subscriber::AckMode;

        let channel = Channel::new("auto_ack_test");
        let sub_id = SubscriberId::new("sub1");

        let mut receiver = channel
            .subscribe_with_ack_mode(sub_id, None, AckMode::Auto)
            .unwrap();

        assert_eq!(receiver.ack_mode(), AckMode::Auto);

        let event = Event::new(
            crate::event::EventType::Created,
            "test",
            EventData::String("auto".to_string()),
        );
        channel.publish(event).unwrap();

        let _received = receiver.recv().await.unwrap();

        // With Auto ack, the message should be immediately acked
        assert_eq!(receiver.unacked_count(), 0);
        assert_eq!(receiver.acked_count(), 1);
    }

    #[tokio::test]
    async fn test_manual_ack_mode() {
        use crate::subscriber::AckMode;

        let channel = Channel::new("manual_ack_test");
        let sub_id = SubscriberId::new("sub1");

        let mut receiver = channel
            .subscribe_with_ack_mode(sub_id, None, AckMode::Manual)
            .unwrap();

        assert_eq!(receiver.ack_mode(), AckMode::Manual);

        // Publish two events
        for i in 0..2 {
            let event = Event::new(crate::event::EventType::Created, "test", EventData::Int(i));
            channel.publish(event).unwrap();
        }

        // Receive both events
        let _ev0 = receiver.recv().await.unwrap();
        let _ev1 = receiver.recv().await.unwrap();

        // Both should be unacked
        assert_eq!(receiver.unacked_count(), 2);
        assert_eq!(receiver.acked_count(), 0);

        // Ack the first message (offset 0)
        receiver.ack(0).unwrap();
        assert_eq!(receiver.unacked_count(), 1);
        assert_eq!(receiver.acked_count(), 1);

        // Ack the second message (offset 1)
        receiver.ack(1).unwrap();
        assert_eq!(receiver.unacked_count(), 0);
        assert_eq!(receiver.acked_count(), 2);

        // Acking again is idempotent
        receiver.ack(0).unwrap();
        assert_eq!(receiver.acked_count(), 2);
    }

    #[tokio::test]
    async fn test_manual_ack_redelivery() {
        use crate::subscriber::AckMode;

        let channel = Channel::new("redeliver_test");
        let sub_id = SubscriberId::new("sub1");

        let mut receiver = channel
            .subscribe_with_ack_mode(sub_id, None, AckMode::Manual)
            .unwrap();

        // Publish events
        for i in 0..3 {
            let event = Event::new(crate::event::EventType::Created, "test", EventData::Int(i));
            channel.publish(event).unwrap();
        }

        // Receive all three
        let _ev0 = receiver.recv().await.unwrap();
        let _ev1 = receiver.recv().await.unwrap();
        let _ev2 = receiver.recv().await.unwrap();

        // Ack only offset 1
        receiver.ack(1).unwrap();

        // Get unacked messages for re-delivery
        let unacked = receiver.get_unacked_messages();
        assert_eq!(unacked.len(), 2);
        // Should be sorted by offset
        assert_eq!(unacked[0].0, 0);
        assert_eq!(unacked[1].0, 2);
    }

    #[tokio::test]
    async fn test_ack_not_found() {
        use crate::subscriber::AckMode;

        let channel = Channel::new("ack_notfound_test");
        let sub_id = SubscriberId::new("sub1");

        let receiver = channel
            .subscribe_with_ack_mode(sub_id, None, AckMode::Manual)
            .unwrap();

        // Acking an offset that was never received
        let result = receiver.ack(999);
        assert!(matches!(result, Err(ChannelError::NotFound(999))));
    }

    #[tokio::test]
    async fn test_none_ack_mode() {
        use crate::subscriber::AckMode;

        let channel = Channel::new("none_ack_test");
        let sub_id = SubscriberId::new("sub1");

        let mut receiver = channel
            .subscribe_with_ack_mode(sub_id, None, AckMode::None)
            .unwrap();

        let event = Event::new(
            crate::event::EventType::Created,
            "test",
            EventData::String("none".to_string()),
        );
        channel.publish(event).unwrap();

        let _received = receiver.recv().await.unwrap();

        // With None ack mode, no tracking at all
        assert_eq!(receiver.unacked_count(), 0);
        assert_eq!(receiver.acked_count(), 0);
    }
}
