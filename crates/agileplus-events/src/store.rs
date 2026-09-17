//! EventStore trait — async append-only event storage.

use agileplus_domain::domain::event::Event;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("Event not found: {0}")]
    NotFound(String),
    #[error("Duplicate sequence: {0}")]
    DuplicateSequence(String),
    #[error("Storage error: {0}")]
    StorageError(String),
    #[error("Invalid hash: {0}")]
    InvalidHash(String),
    #[error("Sequence gap: expected {expected}, got {actual}")]
    SequenceGap { expected: i64, actual: i64 },
}

#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append a new event; returns the assigned sequence number.
    async fn append(&self, event: &Event) -> Result<i64, EventError>;

    /// All events for an entity, ascending by sequence.
    async fn get_events(&self, entity_type: &str, entity_id: i64)
    -> Result<Vec<Event>, EventError>;

    /// Events from a specific sequence onward (exclusive).
    async fn get_events_since(
        &self,
        entity_type: &str,
        entity_id: i64,
        sequence: i64,
    ) -> Result<Vec<Event>, EventError>;

    /// Events within a time range.
    async fn get_events_by_range(
        &self,
        entity_type: &str,
        entity_id: i64,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Event>, EventError>;

    /// Latest event sequence number for an entity (0 if none).
    async fn get_latest_sequence(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<i64, EventError>;

    /// Events by event type, ascending by sequence.
    async fn get_events_by_type(&self, event_type: &str) -> Result<Vec<Event>, EventError> {
        Err(EventError::StorageError(format!(
            "get_events_by_type is not implemented for {event_type}"
        )))
    }
}

#[derive(Debug, Default)]
pub struct InMemoryEventStore {
    events: RwLock<Vec<Event>>,
    sequences: RwLock<HashMap<(String, i64), i64>>,
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn append(&self, event: &Event) -> Result<i64, EventError> {
        let key = (event.entity_type.clone(), event.entity_id);
        let sequence = {
            let mut sequences = self.sequences.write().await;
            let next = sequences.get(&key).copied().unwrap_or(0) + 1;
            sequences.insert(key, next);
            next
        };

        let mut stored = event.clone();
        stored.sequence = sequence;
        self.events.write().await.push(stored);
        Ok(sequence)
    }

    async fn get_events(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<Vec<Event>, EventError> {
        let mut events: Vec<_> = self
            .events
            .read()
            .await
            .iter()
            .filter(|event| event.entity_type == entity_type && event.entity_id == entity_id)
            .cloned()
            .collect();
        events.sort_by_key(|event| event.sequence);
        Ok(events)
    }

    async fn get_events_since(
        &self,
        entity_type: &str,
        entity_id: i64,
        sequence: i64,
    ) -> Result<Vec<Event>, EventError> {
        Ok(self
            .get_events(entity_type, entity_id)
            .await?
            .into_iter()
            .filter(|event| event.sequence > sequence)
            .collect())
    }

    async fn get_events_by_range(
        &self,
        entity_type: &str,
        entity_id: i64,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<Event>, EventError> {
        Ok(self
            .get_events(entity_type, entity_id)
            .await?
            .into_iter()
            .filter(|event| event.timestamp >= from && event.timestamp <= to)
            .collect())
    }

    async fn get_latest_sequence(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<i64, EventError> {
        Ok(self
            .sequences
            .read()
            .await
            .get(&(entity_type.to_string(), entity_id))
            .copied()
            .unwrap_or(0))
    }

    async fn get_events_by_type(&self, event_type: &str) -> Result<Vec<Event>, EventError> {
        let mut events: Vec<_> = self
            .events
            .read()
            .await
            .iter()
            .filter(|event| event.event_type == event_type)
            .cloned()
            .collect();
        events.sort_by_key(|event| event.sequence);
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(entity_type: &str, entity_id: i64, event_type: &str) -> Event {
        Event::new(
            entity_type,
            entity_id,
            event_type,
            serde_json::json!({}),
            "test",
        )
    }

    #[tokio::test]
    async fn in_memory_append_assigns_per_entity_sequence() {
        let store = InMemoryEventStore::new();

        assert_eq!(
            store.append(&event("Feature", 1, "created")).await.unwrap(),
            1
        );
        assert_eq!(
            store.append(&event("Feature", 1, "updated")).await.unwrap(),
            2
        );
        assert_eq!(
            store.append(&event("Feature", 2, "created")).await.unwrap(),
            1
        );

        assert_eq!(store.get_latest_sequence("Feature", 1).await.unwrap(), 2);
        assert_eq!(store.get_latest_sequence("Feature", 2).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn in_memory_get_events_since_filters_sequence() {
        let store = InMemoryEventStore::new();
        store.append(&event("Feature", 1, "created")).await.unwrap();
        store.append(&event("Feature", 1, "updated")).await.unwrap();

        let events = store.get_events_since("Feature", 1, 1).await.unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "updated");
    }

    #[tokio::test]
    async fn in_memory_get_events_by_type_filters_type() {
        let store = InMemoryEventStore::new();
        store.append(&event("Feature", 1, "created")).await.unwrap();
        store
            .append(&event("WorkPackage", 1, "created"))
            .await
            .unwrap();
        store.append(&event("Feature", 1, "updated")).await.unwrap();

        let events = store.get_events_by_type("created").await.unwrap();

        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|event| event.event_type == "created"));
    }
}
