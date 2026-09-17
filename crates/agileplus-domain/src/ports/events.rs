// SPDX-License-Identifier: MIT OR Apache-2.0
//! Domain event publisher port.

use crate::error::DomainError;

/// Application-level domain events emitted AFTER persistence.
#[derive(Debug, Clone)]
pub enum DomainEvent {
    FeatureCreated {
        id: i64,
        slug: String,
    },
    FeatureStateAdvanced {
        id: i64,
        from: String,
        to: String,
    },
    StoryCreated {
        id: i64,
        epic_id: i64,
        title: String,
    },
    StoryStatusChanged {
        id: i64,
        from: String,
        to: String,
    },
    EpicCreated {
        id: i64,
        project_id: i64,
        title: String,
    },
}

/// Output port for publishing domain events to an event bus / message broker.
pub trait DomainEventPublisher: Send + Sync {
    fn publish(&self, event: DomainEvent) -> Result<(), DomainError>;
}
