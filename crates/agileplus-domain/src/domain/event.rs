//! `Event` — the core domain event type for AgilePlus event sourcing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// An append-only domain event in the AgilePlus event store.
///
/// All events carry a SHA-256 hash that chains to the previous event,
/// enabling tamper-evident audit trails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event ID (assigned by the event store on append).
    pub id: i64,
    /// Type of entity this event belongs to (e.g., "Feature", "WorkPackage").
    pub entity_type: String,
    /// ID of the entity this event belongs to.
    pub entity_id: i64,
    /// Kind of event (e.g., "created", "transitioned", "assigned").
    pub event_type: String,
    /// JSON payload with event-specific data.
    pub payload: serde_json::Value,
    /// Actor who caused this event.
    pub actor: String,
    /// When this event occurred.
    pub timestamp: DateTime<Utc>,
    /// SHA-256 hash of the previous event in the chain.
    pub prev_hash: [u8; 32],
    /// SHA-256 hash of this event (computed over event fields + prev_hash).
    pub hash: [u8; 32],
    /// Monotonically increasing sequence number within this entity's stream.
    pub sequence: i64,
}

impl Event {
    /// Create a new unsaved event.  Hash and sequence are assigned by the store.
    pub fn new(
        entity_type: &str,
        entity_id: i64,
        event_type: &str,
        payload: serde_json::Value,
        actor: &str,
    ) -> Self {
        Self {
            id: 0,
            entity_type: entity_type.to_owned(),
            entity_id,
            event_type: event_type.to_owned(),
            payload,
            actor: actor.to_owned(),
            timestamp: Utc::now(),
            prev_hash: [0u8; 32],
            hash: [0u8; 32],
            sequence: 0,
        }
    }
}
