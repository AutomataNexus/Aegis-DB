//! Aegis Streaming Engine
//!
//! Core engine for real-time event streaming.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::cdc::{CdcConfig, ChangeEvent};
use crate::channel::{Channel, ChannelConfig, ChannelError, ChannelId, ChannelReceiver};
use crate::event::{Event, EventFilter};
use crate::subscriber::{ConsumerGroup, Subscriber, SubscriberId, Subscription};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

// =============================================================================
// Engine Configuration
// =============================================================================

/// Configuration for the streaming engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub max_channels: usize,
    pub max_subscribers: usize,
    pub default_channel_config: ChannelConfig,
    pub cdc_config: CdcConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_channels: 1000,
            max_subscribers: 10000,
            default_channel_config: ChannelConfig::default(),
            cdc_config: CdcConfig::default(),
        }
    }
}

// =============================================================================
// Streaming Engine
// =============================================================================

/// The main streaming engine for pub/sub and CDC.
pub struct StreamingEngine {
    config: EngineConfig,
    channels: RwLock<HashMap<ChannelId, Channel>>,
    subscribers: RwLock<HashMap<SubscriberId, Subscriber>>,
    consumer_groups: Arc<RwLock<HashMap<String, ConsumerGroup>>>,
    stats: RwLock<EngineStats>,
}

impl StreamingEngine {
    /// Create a new streaming engine.
    pub fn new() -> Self {
        Self::with_config(EngineConfig::default())
    }

    /// Create an engine with custom configuration.
    pub fn with_config(config: EngineConfig) -> Self {
        Self {
            config,
            channels: RwLock::new(HashMap::new()),
            subscribers: RwLock::new(HashMap::new()),
            consumer_groups: Arc::new(RwLock::new(HashMap::new())),
            stats: RwLock::new(EngineStats::default()),
        }
    }

    // -------------------------------------------------------------------------
    // Channel Management
    // -------------------------------------------------------------------------

    /// Create a new channel.
    pub fn create_channel(&self, id: impl Into<ChannelId>) -> Result<(), EngineError> {
        let id = id.into();
        let mut channels = self
            .channels
            .write()
            .expect("channels RwLock poisoned in create_channel");

        if channels.len() >= self.config.max_channels {
            return Err(EngineError::TooManyChannels);
        }

        if channels.contains_key(&id) {
            return Err(EngineError::ChannelExists(id));
        }

        let channel = Channel::with_config(id.clone(), self.config.default_channel_config.clone());
        channels.insert(id, channel);

        Ok(())
    }

    /// Create a channel with custom configuration.
    pub fn create_channel_with_config(
        &self,
        id: impl Into<ChannelId>,
        config: ChannelConfig,
    ) -> Result<(), EngineError> {
        let id = id.into();
        let mut channels = self
            .channels
            .write()
            .expect("channels RwLock poisoned in create_channel_with_config");

        if channels.len() >= self.config.max_channels {
            return Err(EngineError::TooManyChannels);
        }

        if channels.contains_key(&id) {
            return Err(EngineError::ChannelExists(id));
        }

        let channel = Channel::with_config(id.clone(), config);
        channels.insert(id, channel);

        Ok(())
    }

    /// Delete a channel.
    pub fn delete_channel(&self, id: &ChannelId) -> Result<(), EngineError> {
        let mut channels = self
            .channels
            .write()
            .expect("channels RwLock poisoned in delete_channel");

        if channels.remove(id).is_none() {
            return Err(EngineError::ChannelNotFound(id.clone()));
        }

        Ok(())
    }

    /// List all channels.
    pub fn list_channels(&self) -> Vec<ChannelId> {
        let channels = self
            .channels
            .read()
            .expect("channels RwLock poisoned in list_channels");
        channels.keys().cloned().collect()
    }

    /// Check if a channel exists.
    pub fn channel_exists(&self, id: &ChannelId) -> bool {
        let channels = self
            .channels
            .read()
            .expect("channels RwLock poisoned in channel_exists");
        channels.contains_key(id)
    }

    // -------------------------------------------------------------------------
    // Publishing
    // -------------------------------------------------------------------------

    /// Publish an event to a channel.
    pub fn publish(&self, channel_id: &ChannelId, event: Event) -> Result<usize, EngineError> {
        let channels = self
            .channels
            .read()
            .expect("channels RwLock poisoned in publish");
        let channel = channels
            .get(channel_id)
            .ok_or_else(|| EngineError::ChannelNotFound(channel_id.clone()))?;

        let receivers = channel.publish(event).map_err(EngineError::Channel)?;

        drop(channels);

        {
            let mut stats = self
                .stats
                .write()
                .expect("stats RwLock poisoned in publish");
            stats.events_published += 1;
        }

        Ok(receivers)
    }

    /// Publish a CDC change event.
    pub fn publish_change(
        &self,
        channel_id: &ChannelId,
        change: ChangeEvent,
    ) -> Result<usize, EngineError> {
        let event = change.to_event();
        self.publish(channel_id, event)
    }

    /// Publish to multiple channels.
    pub fn publish_to_many(
        &self,
        channel_ids: &[ChannelId],
        event: Event,
    ) -> HashMap<ChannelId, Result<usize, EngineError>> {
        let mut results = HashMap::new();

        for id in channel_ids {
            results.insert(id.clone(), self.publish(id, event.clone()));
        }

        results
    }

    // -------------------------------------------------------------------------
    // Subscribing
    // -------------------------------------------------------------------------

    /// Subscribe to a channel.
    pub fn subscribe(
        &self,
        channel_id: &ChannelId,
        subscriber_id: impl Into<SubscriberId>,
    ) -> Result<ChannelReceiver, EngineError> {
        let subscriber_id = subscriber_id.into();
        let channels = self
            .channels
            .read()
            .expect("channels RwLock poisoned in subscribe");
        let channel = channels
            .get(channel_id)
            .ok_or_else(|| EngineError::ChannelNotFound(channel_id.clone()))?;

        let receiver = channel
            .subscribe(subscriber_id.clone())
            .map_err(EngineError::Channel)?;

        drop(channels);

        self.ensure_subscriber(&subscriber_id, channel_id);

        {
            let mut stats = self
                .stats
                .write()
                .expect("stats RwLock poisoned in subscribe");
            stats.active_subscriptions += 1;
        }

        Ok(receiver)
    }

    /// Subscribe with a filter.
    pub fn subscribe_with_filter(
        &self,
        channel_id: &ChannelId,
        subscriber_id: impl Into<SubscriberId>,
        filter: EventFilter,
    ) -> Result<ChannelReceiver, EngineError> {
        let subscriber_id = subscriber_id.into();
        let channels = self
            .channels
            .read()
            .expect("channels RwLock poisoned in subscribe_with_filter");
        let channel = channels
            .get(channel_id)
            .ok_or_else(|| EngineError::ChannelNotFound(channel_id.clone()))?;

        let receiver = channel
            .subscribe_with_filter(subscriber_id.clone(), filter)
            .map_err(EngineError::Channel)?;

        drop(channels);

        self.ensure_subscriber(&subscriber_id, channel_id);

        {
            let mut stats = self
                .stats
                .write()
                .expect("stats RwLock poisoned in subscribe_with_filter");
            stats.active_subscriptions += 1;
        }

        Ok(receiver)
    }

    /// Unsubscribe from a channel.
    pub fn unsubscribe(&self, channel_id: &ChannelId, subscriber_id: &SubscriberId) {
        let channels = self
            .channels
            .read()
            .expect("channels RwLock poisoned in unsubscribe");
        if let Some(channel) = channels.get(channel_id) {
            channel.unsubscribe(subscriber_id);
        }

        let mut stats = self
            .stats
            .write()
            .expect("stats RwLock poisoned in unsubscribe");
        stats.active_subscriptions = stats.active_subscriptions.saturating_sub(1);
    }

    fn ensure_subscriber(&self, subscriber_id: &SubscriberId, channel_id: &ChannelId) {
        let mut subscribers = self
            .subscribers
            .write()
            .expect("subscribers RwLock poisoned in ensure_subscriber");

        let subscriber = subscribers
            .entry(subscriber_id.clone())
            .or_insert_with(|| Subscriber::new(subscriber_id.clone()));

        let mut subscription = Subscription::new(subscriber_id.clone());
        subscription.add_channel(channel_id.clone());
        subscriber.add_subscription(subscription);
    }

    // -------------------------------------------------------------------------
    // Subscriber Management
    // -------------------------------------------------------------------------

    /// Get a subscriber.
    pub fn get_subscriber(&self, id: &SubscriberId) -> Option<Subscriber> {
        let subscribers = self
            .subscribers
            .read()
            .expect("subscribers RwLock poisoned in get_subscriber");
        subscribers.get(id).cloned()
    }

    /// List all subscribers.
    pub fn list_subscribers(&self) -> Vec<SubscriberId> {
        let subscribers = self
            .subscribers
            .read()
            .expect("subscribers RwLock poisoned in list_subscribers");
        subscribers.keys().cloned().collect()
    }

    /// Remove a subscriber.
    pub fn remove_subscriber(&self, id: &SubscriberId) {
        let mut subscribers = self
            .subscribers
            .write()
            .expect("subscribers RwLock poisoned in remove_subscriber");
        subscribers.remove(id);
    }

    // -------------------------------------------------------------------------
    // Consumer Group Management
    // -------------------------------------------------------------------------

    /// Create a new consumer group with the given ID.
    pub fn create_consumer_group(&self, group_id: impl Into<String>) -> Result<(), EngineError> {
        let group_id = group_id.into();
        let mut groups = self
            .consumer_groups
            .write()
            .expect("consumer_groups RwLock poisoned in create_consumer_group");

        if groups.contains_key(&group_id) {
            return Err(EngineError::ConsumerGroupExists(group_id));
        }

        groups.insert(group_id.clone(), ConsumerGroup::new(group_id));
        Ok(())
    }

    /// Delete a consumer group.
    pub fn delete_consumer_group(&self, group_id: &str) -> Result<(), EngineError> {
        let mut groups = self
            .consumer_groups
            .write()
            .expect("consumer_groups RwLock poisoned in delete_consumer_group");

        if groups.remove(group_id).is_none() {
            return Err(EngineError::ConsumerGroupNotFound(group_id.to_string()));
        }

        Ok(())
    }

    /// Join a consumer group. The subscriber is added as a member with the given
    /// set of assigned channels.
    pub fn join_consumer_group(
        &self,
        group_id: &str,
        subscriber_id: SubscriberId,
        channels: HashSet<String>,
    ) -> Result<(), EngineError> {
        let mut groups = self
            .consumer_groups
            .write()
            .expect("consumer_groups RwLock poisoned in join_consumer_group");

        let group = groups
            .get_mut(group_id)
            .ok_or_else(|| EngineError::ConsumerGroupNotFound(group_id.to_string()))?;

        group.add_member(subscriber_id, channels);
        Ok(())
    }

    /// Leave a consumer group. Returns the channels that were assigned to the
    /// subscriber, or an error if the group or member was not found.
    pub fn leave_consumer_group(
        &self,
        group_id: &str,
        subscriber_id: &SubscriberId,
    ) -> Result<HashSet<String>, EngineError> {
        let mut groups = self
            .consumer_groups
            .write()
            .expect("consumer_groups RwLock poisoned in leave_consumer_group");

        let group = groups
            .get_mut(group_id)
            .ok_or_else(|| EngineError::ConsumerGroupNotFound(group_id.to_string()))?;

        group.remove_member(subscriber_id).ok_or_else(|| {
            EngineError::SubscriberNotInGroup(subscriber_id.clone(), group_id.to_string())
        })
    }

    /// Commit an offset for a channel within a consumer group.
    pub fn commit_offset(
        &self,
        group_id: &str,
        channel_name: impl Into<String>,
        offset: u64,
    ) -> Result<(), EngineError> {
        let mut groups = self
            .consumer_groups
            .write()
            .expect("consumer_groups RwLock poisoned in commit_offset");

        let group = groups
            .get_mut(group_id)
            .ok_or_else(|| EngineError::ConsumerGroupNotFound(group_id.to_string()))?;

        group.commit_offset(channel_name, offset);
        Ok(())
    }

    /// Get the committed offset for a channel within a consumer group.
    pub fn get_committed_offset(
        &self,
        group_id: &str,
        channel_name: &str,
    ) -> Result<Option<u64>, EngineError> {
        let groups = self
            .consumer_groups
            .read()
            .expect("consumer_groups RwLock poisoned in get_committed_offset");

        let group = groups
            .get(group_id)
            .ok_or_else(|| EngineError::ConsumerGroupNotFound(group_id.to_string()))?;

        Ok(group.get_offset(channel_name))
    }

    /// List all consumer groups.
    pub fn list_consumer_groups(&self) -> Vec<String> {
        let groups = self
            .consumer_groups
            .read()
            .expect("consumer_groups RwLock poisoned in list_consumer_groups");
        groups.keys().cloned().collect()
    }

    /// Get a snapshot of a consumer group.
    pub fn get_consumer_group(&self, group_id: &str) -> Option<ConsumerGroup> {
        let groups = self
            .consumer_groups
            .read()
            .expect("consumer_groups RwLock poisoned in get_consumer_group");
        groups.get(group_id).cloned()
    }

    // -------------------------------------------------------------------------
    // History and Replay
    // -------------------------------------------------------------------------

    /// Get recent events from a channel.
    pub fn get_history(
        &self,
        channel_id: &ChannelId,
        count: usize,
    ) -> Result<Vec<Event>, EngineError> {
        let channels = self
            .channels
            .read()
            .expect("channels RwLock poisoned in get_history");
        let channel = channels
            .get(channel_id)
            .ok_or_else(|| EngineError::ChannelNotFound(channel_id.clone()))?;

        Ok(channel.get_history(count))
    }

    /// Get events after a timestamp.
    pub fn get_history_after(
        &self,
        channel_id: &ChannelId,
        timestamp: u64,
    ) -> Result<Vec<Event>, EngineError> {
        let channels = self
            .channels
            .read()
            .expect("channels RwLock poisoned in get_history_after");
        let channel = channels
            .get(channel_id)
            .ok_or_else(|| EngineError::ChannelNotFound(channel_id.clone()))?;

        Ok(channel.get_history_after(timestamp))
    }

    // -------------------------------------------------------------------------
    // Statistics
    // -------------------------------------------------------------------------

    /// Get engine statistics.
    pub fn stats(&self) -> EngineStats {
        let stats = self.stats.read().expect("stats RwLock poisoned in stats");
        stats.clone()
    }

    /// Reset statistics.
    pub fn reset_stats(&self) {
        let mut stats = self
            .stats
            .write()
            .expect("stats RwLock poisoned in reset_stats");
        *stats = EngineStats::default();
    }

    /// Get channel statistics.
    pub fn channel_stats(&self, id: &ChannelId) -> Option<crate::channel::ChannelStats> {
        let channels = self
            .channels
            .read()
            .expect("channels RwLock poisoned in channel_stats");
        channels.get(id).map(|c| c.stats())
    }
}

impl Default for StreamingEngine {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Engine Statistics
// =============================================================================

/// Statistics for the streaming engine.
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    pub events_published: u64,
    pub active_subscriptions: usize,
    pub channels_created: usize,
}

// =============================================================================
// Engine Error
// =============================================================================

/// Errors that can occur in the streaming engine.
#[derive(Debug, Clone)]
pub enum EngineError {
    ChannelExists(ChannelId),
    ChannelNotFound(ChannelId),
    TooManyChannels,
    TooManySubscribers,
    Channel(ChannelError),
    ConsumerGroupExists(String),
    ConsumerGroupNotFound(String),
    SubscriberNotInGroup(SubscriberId, String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChannelExists(id) => write!(f, "Channel already exists: {}", id),
            Self::ChannelNotFound(id) => write!(f, "Channel not found: {}", id),
            Self::TooManyChannels => write!(f, "Maximum channels reached"),
            Self::TooManySubscribers => write!(f, "Maximum subscribers reached"),
            Self::Channel(err) => write!(f, "Channel error: {}", err),
            Self::ConsumerGroupExists(id) => write!(f, "Consumer group already exists: {}", id),
            Self::ConsumerGroupNotFound(id) => write!(f, "Consumer group not found: {}", id),
            Self::SubscriberNotInGroup(sub_id, group_id) => {
                write!(
                    f,
                    "Subscriber {} is not in consumer group {}",
                    sub_id, group_id
                )
            }
        }
    }
}

impl std::error::Error for EngineError {}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventData, EventType};

    #[test]
    fn test_engine_creation() {
        let engine = StreamingEngine::new();
        assert!(engine.list_channels().is_empty());
    }

    #[test]
    fn test_channel_management() {
        let engine = StreamingEngine::new();

        engine.create_channel("events").unwrap();
        assert!(engine.channel_exists(&ChannelId::new("events")));

        let channels = engine.list_channels();
        assert_eq!(channels.len(), 1);

        engine.delete_channel(&ChannelId::new("events")).unwrap();
        assert!(!engine.channel_exists(&ChannelId::new("events")));
    }

    #[tokio::test]
    async fn test_publish_subscribe() {
        let engine = StreamingEngine::new();
        engine.create_channel("test").unwrap();

        let channel_id = ChannelId::new("test");
        let mut receiver = engine.subscribe(&channel_id, "sub1").unwrap();

        let event = Event::new(
            EventType::Created,
            "source",
            EventData::String("hello".to_string()),
        );
        engine.publish(&channel_id, event).unwrap();

        let received = receiver.recv().await.unwrap();
        assert_eq!(received.source, "source");
    }

    #[test]
    fn test_duplicate_channel() {
        let engine = StreamingEngine::new();

        engine.create_channel("test").unwrap();
        let result = engine.create_channel("test");

        assert!(matches!(result, Err(EngineError::ChannelExists(_))));
    }

    #[test]
    fn test_stats() {
        let engine = StreamingEngine::new();
        engine.create_channel("test").unwrap();

        let channel_id = ChannelId::new("test");
        engine.subscribe(&channel_id, "sub1").unwrap();

        let event = Event::new(EventType::Created, "source", EventData::Null);
        engine.publish(&channel_id, event).unwrap();

        let stats = engine.stats();
        assert_eq!(stats.events_published, 1);
        assert_eq!(stats.active_subscriptions, 1);
    }

    #[test]
    fn test_history() {
        let config = EngineConfig {
            default_channel_config: ChannelConfig {
                persistent: true,
                retention_count: 100,
                ..Default::default()
            },
            ..Default::default()
        };

        let engine = StreamingEngine::with_config(config);
        engine.create_channel("history").unwrap();

        let channel_id = ChannelId::new("history");

        for i in 0..5 {
            let event = Event::new(EventType::Created, "test", EventData::Int(i));
            engine.publish(&channel_id, event).unwrap();
        }

        let history = engine.get_history(&channel_id, 10).unwrap();
        assert_eq!(history.len(), 5);
    }

    #[test]
    fn test_consumer_group_create_delete() {
        let engine = StreamingEngine::new();

        engine.create_consumer_group("group1").unwrap();
        assert_eq!(engine.list_consumer_groups().len(), 1);

        // Duplicate creation should fail
        let result = engine.create_consumer_group("group1");
        assert!(matches!(result, Err(EngineError::ConsumerGroupExists(_))));

        engine.delete_consumer_group("group1").unwrap();
        assert!(engine.list_consumer_groups().is_empty());

        // Deleting non-existent group should fail
        let result = engine.delete_consumer_group("group1");
        assert!(matches!(result, Err(EngineError::ConsumerGroupNotFound(_))));
    }

    #[test]
    fn test_consumer_group_join_leave() {
        let engine = StreamingEngine::new();
        engine.create_consumer_group("group1").unwrap();

        let sub1 = SubscriberId::new("sub1");
        let sub2 = SubscriberId::new("sub2");

        let mut channels1 = std::collections::HashSet::new();
        channels1.insert("events".to_string());

        let mut channels2 = std::collections::HashSet::new();
        channels2.insert("logs".to_string());

        engine
            .join_consumer_group("group1", sub1.clone(), channels1)
            .unwrap();
        engine
            .join_consumer_group("group1", sub2.clone(), channels2)
            .unwrap();

        let group = engine.get_consumer_group("group1").unwrap();
        assert_eq!(group.member_count(), 2);
        assert!(group.is_member(&sub1));

        // Leave
        let removed_channels = engine.leave_consumer_group("group1", &sub1).unwrap();
        assert!(removed_channels.contains("events"));

        let group = engine.get_consumer_group("group1").unwrap();
        assert_eq!(group.member_count(), 1);
        assert!(!group.is_member(&sub1));

        // Leaving again should fail
        let result = engine.leave_consumer_group("group1", &sub1);
        assert!(matches!(
            result,
            Err(EngineError::SubscriberNotInGroup(_, _))
        ));
    }

    #[test]
    fn test_consumer_group_join_nonexistent() {
        let engine = StreamingEngine::new();

        let result = engine.join_consumer_group(
            "nonexistent",
            SubscriberId::new("sub1"),
            std::collections::HashSet::new(),
        );
        assert!(matches!(result, Err(EngineError::ConsumerGroupNotFound(_))));
    }

    #[test]
    fn test_consumer_group_offset_tracking() {
        let engine = StreamingEngine::new();
        engine.create_consumer_group("group1").unwrap();

        // No offset committed yet
        let offset = engine.get_committed_offset("group1", "events").unwrap();
        assert_eq!(offset, None);

        // Commit offset
        engine.commit_offset("group1", "events", 42).unwrap();
        let offset = engine.get_committed_offset("group1", "events").unwrap();
        assert_eq!(offset, Some(42));

        // Update offset
        engine.commit_offset("group1", "events", 100).unwrap();
        let offset = engine.get_committed_offset("group1", "events").unwrap();
        assert_eq!(offset, Some(100));

        // Different channel
        engine.commit_offset("group1", "logs", 5).unwrap();
        let offset = engine.get_committed_offset("group1", "logs").unwrap();
        assert_eq!(offset, Some(5));
        // Original channel offset unchanged
        let offset = engine.get_committed_offset("group1", "events").unwrap();
        assert_eq!(offset, Some(100));
    }

    #[test]
    fn test_consumer_group_offset_nonexistent_group() {
        let engine = StreamingEngine::new();

        let result = engine.commit_offset("nonexistent", "events", 10);
        assert!(matches!(result, Err(EngineError::ConsumerGroupNotFound(_))));

        let result = engine.get_committed_offset("nonexistent", "events");
        assert!(matches!(result, Err(EngineError::ConsumerGroupNotFound(_))));
    }

    #[test]
    fn test_get_consumer_group() {
        let engine = StreamingEngine::new();

        assert!(engine.get_consumer_group("nonexistent").is_none());

        engine.create_consumer_group("group1").unwrap();
        let group = engine.get_consumer_group("group1");
        assert!(group.is_some());
        assert_eq!(group.unwrap().group_id, "group1");
    }
}
