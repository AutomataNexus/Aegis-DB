//! Aegis Streaming Channels
//!
//! Pub/sub channels for event distribution.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::event::{Event, EventFilter};
use crate::subscriber::SubscriberId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;
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
            let mut history = self.history.write().unwrap();
            history.push_back(event.clone());

            while history.len() > self.config.retention_count {
                history.pop_front();
            }
        }

        let receivers = self.sender.send(event).unwrap_or(0);

        {
            let mut stats = self.stats.write().unwrap();
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
        let subscribers = self.subscribers.read().unwrap();
        if subscribers.len() >= self.config.max_subscribers {
            return Err(ChannelError::TooManySubscribers);
        }
        drop(subscribers);

        let receiver = self.sender.subscribe();

        {
            let mut subscribers = self.subscribers.write().unwrap();
            subscribers.insert(
                subscriber_id.clone(),
                SubscriberInfo {
                    filter: None,
                    subscribed_at: current_timestamp(),
                },
            );
        }

        {
            let mut stats = self.stats.write().unwrap();
            stats.subscriber_count += 1;
        }

        Ok(ChannelReceiver {
            receiver,
            filter: None,
        })
    }

    /// Subscribe with a filter.
    pub fn subscribe_with_filter(
        &self,
        subscriber_id: SubscriberId,
        filter: EventFilter,
    ) -> Result<ChannelReceiver, ChannelError> {
        let subscribers = self.subscribers.read().unwrap();
        if subscribers.len() >= self.config.max_subscribers {
            return Err(ChannelError::TooManySubscribers);
        }
        drop(subscribers);

        let receiver = self.sender.subscribe();

        {
            let mut subscribers = self.subscribers.write().unwrap();
            subscribers.insert(
                subscriber_id.clone(),
                SubscriberInfo {
                    filter: Some(filter.clone()),
                    subscribed_at: current_timestamp(),
                },
            );
        }

        {
            let mut stats = self.stats.write().unwrap();
            stats.subscriber_count += 1;
        }

        Ok(ChannelReceiver {
            receiver,
            filter: Some(filter),
        })
    }

    /// Unsubscribe from the channel.
    pub fn unsubscribe(&self, subscriber_id: &SubscriberId) {
        let mut subscribers = self.subscribers.write().unwrap();
        if subscribers.remove(subscriber_id).is_some() {
            let mut stats = self.stats.write().unwrap();
            stats.subscriber_count = stats.subscriber_count.saturating_sub(1);
        }
    }

    /// Get the number of subscribers.
    pub fn subscriber_count(&self) -> usize {
        let subscribers = self.subscribers.read().unwrap();
        subscribers.len()
    }

    /// Get recent events from history.
    pub fn get_history(&self, count: usize) -> Vec<Event> {
        let history = self.history.read().unwrap();
        history.iter().rev().take(count).cloned().collect()
    }

    /// Get events from history after a timestamp.
    pub fn get_history_after(&self, timestamp: u64) -> Vec<Event> {
        let history = self.history.read().unwrap();
        history
            .iter()
            .filter(|e| e.timestamp > timestamp)
            .cloned()
            .collect()
    }

    /// Get channel statistics.
    pub fn stats(&self) -> ChannelStats {
        let stats = self.stats.read().unwrap();
        stats.clone()
    }

    /// Clear history.
    pub fn clear_history(&self) {
        let mut history = self.history.write().unwrap();
        history.clear();
    }
}

// =============================================================================
// Channel Receiver
// =============================================================================

/// Receiver for channel events.
pub struct ChannelReceiver {
    receiver: broadcast::Receiver<Event>,
    filter: Option<EventFilter>,
}

impl ChannelReceiver {
    /// Receive the next event.
    pub async fn recv(&mut self) -> Result<Event, ChannelError> {
        loop {
            match self.receiver.recv().await {
                Ok(event) => {
                    if let Some(ref filter) = self.filter {
                        if !event.matches(filter) {
                            continue;
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
}

impl std::fmt::Display for ChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManySubscribers => write!(f, "Maximum subscribers reached"),
            Self::Closed => write!(f, "Channel is closed"),
            Self::Lagged(n) => write!(f, "Receiver lagged by {} messages", n),
            Self::SendFailed => write!(f, "Failed to send event"),
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
            let event = Event::new(
                crate::event::EventType::Created,
                "test",
                EventData::Int(i),
            );
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
}
