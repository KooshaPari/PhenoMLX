//! `Snapshot` — point-in-time state capture for fast aggregate reloading.

use serde::{Deserialize, Serialize};

/// A snapshot captures aggregate state at a specific event sequence,
/// enabling fast reloading without replaying the full event history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Type of entity this snapshot belongs to.
    pub entity_type: String,
    /// ID of the entity.
    pub entity_id: i64,
    /// Serialized aggregate state.
    pub state: serde_json::Value,
    /// The last event sequence included in this snapshot.
    pub event_sequence: i64,
}

impl Snapshot {
    pub fn new(
        entity_type: &str,
        entity_id: i64,
        state: serde_json::Value,
        event_sequence: i64,
    ) -> Self {
        Self {
            entity_type: entity_type.to_owned(),
            entity_id,
            state,
            event_sequence,
        }
    }
}
