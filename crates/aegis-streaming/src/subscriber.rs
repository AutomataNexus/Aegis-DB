//! Aegis Streaming Subscribers
//!
//! Subscriber management for event subscriptions.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::channel::ChannelId;
use crate::event::EventFilter;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// Subscriber ID
// =============================================================================

/// Unique identifier for a subscriber.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubscriberId(pub String);

impl SubscriberId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn generate() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self(format!("sub_{:032x}", timestamp))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SubscriberId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for SubscriberId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SubscriberId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// =============================================================================
// Subscription
// =============================================================================

/// A subscription to one or more channels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: SubscriberId,
    pub channels: HashSet<ChannelId>,
    pub filter: Option<EventFilter>,
    pub created_at: u64,
    pub active: bool,
    pub metadata: SubscriptionMetadata,
}

impl Subscription {
    /// Create a new subscription.
    pub fn new(id: impl Into<SubscriberId>) -> Self {
        Self {
            id: id.into(),
            channels: HashSet::new(),
            filter: None,
            created_at: current_timestamp(),
            active: true,
            metadata: SubscriptionMetadata::default(),
        }
    }

    /// Add a channel to the subscription.
    pub fn add_channel(&mut self, channel: impl Into<ChannelId>) {
        self.channels.insert(channel.into());
    }

    /// Remove a channel from the subscription.
    pub fn remove_channel(&mut self, channel: &ChannelId) {
        self.channels.remove(channel);
    }

    /// Set the event filter.
    pub fn with_filter(mut self, filter: EventFilter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Check if subscribed to a channel.
    pub fn is_subscribed_to(&self, channel: &ChannelId) -> bool {
        self.channels.contains(channel)
    }

    /// Deactivate the subscription.
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Reactivate the subscription.
    pub fn activate(&mut self) {
        self.active = true;
    }
}

// =============================================================================
// Subscription Metadata
// =============================================================================

/// Metadata for a subscription.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubscriptionMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub delivery_mode: DeliveryMode,
    pub ack_mode: AckMode,
}

impl SubscriptionMetadata {
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_delivery_mode(mut self, mode: DeliveryMode) -> Self {
        self.delivery_mode = mode;
        self
    }

    pub fn with_ack_mode(mut self, mode: AckMode) -> Self {
        self.ack_mode = mode;
        self
    }
}

// =============================================================================
// Delivery Mode
// =============================================================================

/// Mode for event delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DeliveryMode {
    /// Events are delivered at most once.
    AtMostOnce,
    /// Events are delivered at least once.
    #[default]
    AtLeastOnce,
    /// Events are delivered exactly once.
    ExactlyOnce,
}

// =============================================================================
// Acknowledgment Mode
// =============================================================================

/// Mode for event acknowledgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AckMode {
    /// Automatic acknowledgment on receive.
    #[default]
    Auto,
    /// Manual acknowledgment required.
    Manual,
    /// No acknowledgment needed.
    None,
}

// =============================================================================
// Subscriber
// =============================================================================

/// A subscriber that receives events.
#[derive(Debug, Clone)]
pub struct Subscriber {
    pub id: SubscriberId,
    pub subscriptions: Vec<Subscription>,
    pub created_at: u64,
    pub last_active: u64,
    pub events_received: u64,
    pub events_acknowledged: u64,
}

impl Subscriber {
    /// Create a new subscriber.
    pub fn new(id: impl Into<SubscriberId>) -> Self {
        let now = current_timestamp();
        Self {
            id: id.into(),
            subscriptions: Vec::new(),
            created_at: now,
            last_active: now,
            events_received: 0,
            events_acknowledged: 0,
        }
    }

    /// Add a subscription.
    pub fn add_subscription(&mut self, subscription: Subscription) {
        self.subscriptions.push(subscription);
    }

    /// Remove a subscription.
    pub fn remove_subscription(&mut self, subscription_id: &SubscriberId) {
        self.subscriptions.retain(|s| &s.id != subscription_id);
    }

    /// Get active subscriptions.
    pub fn active_subscriptions(&self) -> Vec<&Subscription> {
        self.subscriptions.iter().filter(|s| s.active).collect()
    }

    /// Record an event received.
    pub fn record_received(&mut self) {
        self.events_received += 1;
        self.last_active = current_timestamp();
    }

    /// Record an event acknowledged.
    pub fn record_acknowledged(&mut self) {
        self.events_acknowledged += 1;
    }

    /// Check if the subscriber is active.
    pub fn is_active(&self) -> bool {
        !self.subscriptions.is_empty()
            && self.subscriptions.iter().any(|s| s.active)
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
    fn test_subscriber_id() {
        let id1 = SubscriberId::generate();
        let id2 = SubscriberId::generate();
        assert_ne!(id1, id2);
        assert!(id1.as_str().starts_with("sub_"));
    }

    #[test]
    fn test_subscription() {
        let mut subscription = Subscription::new("sub1");
        subscription.add_channel("channel1");
        subscription.add_channel("channel2");

        assert!(subscription.is_subscribed_to(&ChannelId::new("channel1")));
        assert!(!subscription.is_subscribed_to(&ChannelId::new("channel3")));
        assert!(subscription.active);

        subscription.deactivate();
        assert!(!subscription.active);
    }

    #[test]
    fn test_subscriber() {
        let mut subscriber = Subscriber::new("user1");

        let mut sub1 = Subscription::new("sub1");
        sub1.add_channel("events");
        subscriber.add_subscription(sub1);

        assert!(subscriber.is_active());
        assert_eq!(subscriber.active_subscriptions().len(), 1);

        subscriber.record_received();
        assert_eq!(subscriber.events_received, 1);
    }

    #[test]
    fn test_subscription_metadata() {
        let metadata = SubscriptionMetadata::default()
            .with_name("Test Subscription")
            .with_description("A test subscription")
            .with_tag("test")
            .with_delivery_mode(DeliveryMode::ExactlyOnce);

        assert_eq!(metadata.name, Some("Test Subscription".to_string()));
        assert_eq!(metadata.delivery_mode, DeliveryMode::ExactlyOnce);
    }
}
