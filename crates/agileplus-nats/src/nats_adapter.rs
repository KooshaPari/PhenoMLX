// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hexagonal adapter: implements the `AsyncEventBus` / `EventBus` ports defined
//! in `agileplus-events` over a live `async-nats` connection.
//!
//! # Subject scheme
//!
//! ```text
//! agileplus.events.<AggregateType>.<event_type>
//! ```
//!
//! Examples:
//! - `agileplus.events.Project.project.created`
//! - `agileplus.events.Story.story.status_changed`
//! - `agileplus.events.WorkPackage.work_package.created`
//!
//! The `<AggregateType>` segment comes from `EventEnvelope::aggregate_type`
//! (e.g. `"Project"`, `"Epic"`).  The `<event_type>` segment comes from
//! `DomainEvent::event_type()` (e.g. `"project.created"`).  Dots within
//! event_type tokens are preserved — NATS treats them as nested subjects,
//! which is intentional and enables fine-grained wildcard subscriptions.
//!
//! # Subscriber side
//!
//! `NatsEventSubscriber` wraps an `async_nats::Subscriber` and delivers each
//! incoming message as a deserialized `EventEnvelope` to a caller-supplied
//! `AsyncEventHandler`.  Subscribe with `NatsEventBus::subscribe_handler`.
//!
//! # Broker-free testing
//!
//! Unit tests in this module do *not* require a live NATS server.  They
//! exercise `derive_subject` and `envelope_to_bytes` directly.  Any test that
//! needs a real broker is gated with `#[ignore]`.

use std::sync::Arc;

use async_trait::async_trait;
use futures_util::StreamExt as _;
use tracing::{debug, error};

use agileplus_events::{AsyncEventBus, AsyncEventHandler, EventEnvelope, EventHandlerError};

use crate::config::NatsConfig;

// ──────────────────────────────────────────────────────────────────────────────
// Error type
// ──────────────────────────────────────────────────────────────────────────────

/// Errors produced by the NATS event-bus adapter.
#[derive(Debug, thiserror::Error)]
pub enum NatsEventBusError {
    #[error("NATS connection error: {0}")]
    Connection(String),
    #[error("NATS publish error: {0}")]
    Publish(String),
    #[error("NATS subscribe error: {0}")]
    Subscribe(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Handler error: {0}")]
    Handler(#[from] EventHandlerError),
}

impl From<NatsEventBusError> for EventHandlerError {
    fn from(e: NatsEventBusError) -> Self {
        EventHandlerError::Transient(e.to_string())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Subject derivation (pure function — easily unit-tested)
// ──────────────────────────────────────────────────────────────────────────────

/// Derive the NATS subject for an `EventEnvelope`.
///
/// Scheme: `{prefix}.events.{aggregate_type}.{event_type}`
///
/// Both `aggregate_type` and `event_type` are taken verbatim from the
/// envelope/payload; callers control the `prefix` via `NatsConfig`.
pub fn derive_subject(prefix: &str, envelope: &EventEnvelope) -> String {
    let aggregate = &envelope.aggregate_type;
    let event_kind = envelope.payload.event_type();
    format!("{prefix}.events.{aggregate}.{event_kind}")
}

// ──────────────────────────────────────────────────────────────────────────────
// Wire encoding
// ──────────────────────────────────────────────────────────────────────────────

/// Serialize an `EventEnvelope` to JSON bytes for wire transport.
pub fn envelope_to_bytes(envelope: &EventEnvelope) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(envelope)
}

/// Deserialize an `EventEnvelope` from JSON bytes received over NATS.
pub fn envelope_from_bytes(bytes: &[u8]) -> Result<EventEnvelope, serde_json::Error> {
    serde_json::from_slice(bytes)
}

// ──────────────────────────────────────────────────────────────────────────────
// NatsEventBus — hexagonal adapter for the AsyncEventBus port
// ──────────────────────────────────────────────────────────────────────────────

/// Hexagonal adapter that publishes `EventEnvelope`s to NATS.
///
/// Implements [`agileplus_events::AsyncEventBus`] so application-layer code
/// depends only on the port, never on `async-nats` directly.
///
/// Construction requires a connected `async_nats::Client`; use
/// `NatsEventBus::connect` for convenience or inject a client directly via
/// `NatsEventBus::from_client`.
pub struct NatsEventBus {
    client: async_nats::Client,
    prefix: String,
}

impl NatsEventBus {
    /// Connect to a NATS server using the provided configuration and return
    /// a ready adapter.
    ///
    /// Requires a live NATS broker — gate integration tests with `#[ignore]`.
    pub async fn connect(config: &NatsConfig) -> Result<Self, NatsEventBusError> {
        let options = {
            let mut opts = async_nats::ConnectOptions::new();
            if let Some(token) = &config.auth_token {
                opts = opts.token(token.clone());
            }
            opts
        };

        let client = options
            .connect(&config.url)
            .await
            .map_err(|e| NatsEventBusError::Connection(e.to_string()))?;

        Ok(Self {
            client,
            prefix: config.subject_prefix.clone(),
        })
    }

    /// Wrap an already-connected `async_nats::Client`.
    pub fn from_client(client: async_nats::Client, prefix: impl Into<String>) -> Self {
        Self {
            client,
            prefix: prefix.into(),
        }
    }

    /// Subscribe to all events for a given aggregate type and dispatch each
    /// incoming envelope to the provided `AsyncEventHandler`.
    ///
    /// The subject pattern used is `{prefix}.events.{aggregate_type}.>`
    /// (matches every event kind for that aggregate).
    ///
    /// Requires a live NATS broker — gate integration tests with `#[ignore]`.
    pub async fn subscribe_handler(
        &self,
        aggregate_type: &str,
        handler: Arc<dyn AsyncEventHandler>,
    ) -> Result<(), NatsEventBusError> {
        let subject = format!("{}.events.{}.>", self.prefix, aggregate_type);
        let mut subscriber = self
            .client
            .subscribe(subject)
            .await
            .map_err(|e| NatsEventBusError::Subscribe(e.to_string()))?;

        tokio::spawn(async move {
            while let Some(msg) = subscriber.next().await {
                match envelope_from_bytes(&msg.payload) {
                    Ok(envelope) => {
                        debug!(
                            event_id = %envelope.id,
                            aggregate = %envelope.aggregate_type,
                            "NATS: received event envelope"
                        );
                        if let Err(e) = handler.handle(&envelope).await {
                            error!(error = %e, "NATS: handler returned error");
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "NATS: failed to deserialize envelope");
                    }
                }
            }
        });

        Ok(())
    }

    /// Return the subject prefix this adapter is using.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Port implementation: AsyncEventBus
// ──────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl AsyncEventBus for NatsEventBus {
    async fn publish(&self, envelope: EventEnvelope) -> Result<(), EventHandlerError> {
        let subject = derive_subject(&self.prefix, &envelope);
        let bytes = envelope_to_bytes(&envelope)
            .map_err(|e| EventHandlerError::Rejected(format!("serialize: {e}")))?;

        debug!(
            subject = %subject,
            event_id = %envelope.id,
            "NATS: publishing event"
        );

        self.client
            .publish(subject, bytes.into())
            .await
            .map_err(|e| EventHandlerError::Transient(format!("nats publish: {e}")))?;

        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests (no live broker required)
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use agileplus_domain::domain::epic::EpicStatus;
    use agileplus_domain::domain::state_machine::FeatureState;
    use agileplus_domain::domain::story::StoryStatus;
    use agileplus_domain::domain::user::{UserRole, UserStatus};
    use agileplus_domain::domain::work_package::WpState;
    use agileplus_events::{
        AggregateId, DomainEvent, EpicCreated, EpicStatusChanged, FeatureCreated, FeatureShipped,
        FeatureStateAdvanced, ProjectArchived, ProjectCreated, ProjectRenamed, StoryAssigned,
        StoryCreated, StoryStatusChanged, UserAdded, UserRoleChanged, UserStatusChanged,
        WorkPackageCreated, WorkPackageStateChanged,
    };

    // ── helpers ───────────────────────────────────────────────────────────────

    fn envelope(id: i64, event: DomainEvent) -> EventEnvelope {
        EventEnvelope::new(AggregateId(id), event)
    }

    // ── subject derivation ────────────────────────────────────────────────────

    #[test]
    fn subject_project_created() {
        let env = envelope(
            1,
            DomainEvent::ProjectCreated(ProjectCreated {
                project_id: 1.into(),
                slug: "s".into(),
                name: "n".into(),
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.Project.project.created"
        );
    }

    #[test]
    fn subject_project_renamed() {
        let env = envelope(
            1,
            DomainEvent::ProjectRenamed(ProjectRenamed {
                project_id: 1.into(),
                old_name: "old".into(),
                new_name: "new".into(),
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.Project.project.renamed"
        );
    }

    #[test]
    fn subject_project_archived() {
        let env = envelope(
            1,
            DomainEvent::ProjectArchived(ProjectArchived {
                project_id: 1.into(),
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.Project.project.archived"
        );
    }

    #[test]
    fn subject_epic_created() {
        let env = envelope(
            2,
            DomainEvent::EpicCreated(EpicCreated {
                epic_id: 2.into(),
                project_id: 1.into(),
                title: "Epic 1".into(),
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.Epic.epic.created"
        );
    }

    #[test]
    fn subject_epic_status_changed() {
        let env = envelope(
            2,
            DomainEvent::EpicStatusChanged(EpicStatusChanged {
                epic_id: 2.into(),
                project_id: 1.into(),
                from: EpicStatus::Backlog,
                to: EpicStatus::Active,
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.Epic.epic.status_changed"
        );
    }

    #[test]
    fn subject_story_created() {
        let env = envelope(
            10,
            DomainEvent::StoryCreated(StoryCreated {
                story_id: 10.into(),
                epic_id: 2.into(),
                project_id: 1.into(),
                title: "Login".into(),
                points: Some(3),
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.Story.story.created"
        );
    }

    #[test]
    fn subject_story_status_changed() {
        let env = envelope(
            10,
            DomainEvent::StoryStatusChanged(StoryStatusChanged {
                story_id: 10.into(),
                epic_id: 2.into(),
                from: StoryStatus::Todo,
                to: StoryStatus::InProgress,
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.Story.story.status_changed"
        );
    }

    #[test]
    fn subject_story_assigned() {
        let env = envelope(
            10,
            DomainEvent::StoryAssigned(StoryAssigned {
                story_id: 10.into(),
                assignee_id: Some(5.into()),
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.Story.story.assigned"
        );
    }

    #[test]
    fn subject_user_added() {
        let env = envelope(
            5,
            DomainEvent::UserAdded(UserAdded {
                user_id: 5.into(),
                display_name: "Alice".into(),
                email: "alice@example.com".into(),
                role: UserRole::Member,
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.User.user.added"
        );
    }

    #[test]
    fn subject_user_role_changed() {
        let env = envelope(
            5,
            DomainEvent::UserRoleChanged(UserRoleChanged {
                user_id: 5.into(),
                old_role: UserRole::Member,
                new_role: UserRole::Admin,
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.User.user.role_changed"
        );
    }

    #[test]
    fn subject_user_status_changed() {
        let env = envelope(
            5,
            DomainEvent::UserStatusChanged(UserStatusChanged {
                user_id: 5.into(),
                from: UserStatus::Active,
                to: UserStatus::Suspended,
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.User.user.status_changed"
        );
    }

    #[test]
    fn subject_feature_created() {
        let env = envelope(
            3,
            DomainEvent::FeatureCreated(FeatureCreated {
                feature_id: 3.into(),
                slug: "feat-login".into(),
                friendly_name: "Login".into(),
                project_id: Some(1.into()),
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.Feature.feature.created"
        );
    }

    #[test]
    fn subject_feature_state_advanced() {
        let env = envelope(
            3,
            DomainEvent::FeatureStateAdvanced(FeatureStateAdvanced {
                feature_id: 3.into(),
                from: FeatureState::Created,
                to: FeatureState::Specified,
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.Feature.feature.state_advanced"
        );
    }

    #[test]
    fn subject_feature_shipped() {
        let env = envelope(
            3,
            DomainEvent::FeatureShipped(FeatureShipped {
                feature_id: 3.into(),
                slug: "feat-login".into(),
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.Feature.feature.shipped"
        );
    }

    #[test]
    fn subject_work_package_created() {
        let env = envelope(
            20,
            DomainEvent::WorkPackageCreated(WorkPackageCreated {
                wp_id: 20.into(),
                feature_id: 3.into(),
                title: "Implement login".into(),
                sequence: 1,
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.WorkPackage.work_package.created"
        );
    }

    #[test]
    fn subject_work_package_state_changed() {
        let env = envelope(
            20,
            DomainEvent::WorkPackageStateChanged(WorkPackageStateChanged {
                wp_id: 20.into(),
                feature_id: 3.into(),
                from: WpState::Planned,
                to: WpState::Doing,
            }),
        );
        assert_eq!(
            derive_subject("agileplus", &env),
            "agileplus.events.WorkPackage.work_package.state_changed"
        );
    }

    #[test]
    fn custom_prefix_is_respected() {
        let env = envelope(
            1,
            DomainEvent::ProjectCreated(ProjectCreated {
                project_id: 1.into(),
                slug: "s".into(),
                name: "n".into(),
            }),
        );
        assert_eq!(
            derive_subject("myapp", &env),
            "myapp.events.Project.project.created"
        );
    }

    #[test]
    fn all_sixteen_subjects_are_unique() {
        use std::collections::HashSet;

        let events: Vec<DomainEvent> = vec![
            DomainEvent::ProjectCreated(ProjectCreated {
                project_id: 1.into(),
                slug: "s".into(),
                name: "n".into(),
            }),
            DomainEvent::ProjectRenamed(ProjectRenamed {
                project_id: 1.into(),
                old_name: "o".into(),
                new_name: "n".into(),
            }),
            DomainEvent::ProjectArchived(ProjectArchived {
                project_id: 1.into(),
            }),
            DomainEvent::EpicCreated(EpicCreated {
                epic_id: 2.into(),
                project_id: 1.into(),
                title: "e".into(),
            }),
            DomainEvent::EpicStatusChanged(EpicStatusChanged {
                epic_id: 2.into(),
                project_id: 1.into(),
                from: EpicStatus::Backlog,
                to: EpicStatus::Active,
            }),
            DomainEvent::StoryCreated(StoryCreated {
                story_id: 10.into(),
                epic_id: 2.into(),
                project_id: 1.into(),
                title: "s".into(),
                points: None,
            }),
            DomainEvent::StoryStatusChanged(StoryStatusChanged {
                story_id: 10.into(),
                epic_id: 2.into(),
                from: StoryStatus::Todo,
                to: StoryStatus::InProgress,
            }),
            DomainEvent::StoryAssigned(StoryAssigned {
                story_id: 10.into(),
                assignee_id: None,
            }),
            DomainEvent::UserAdded(UserAdded {
                user_id: 5.into(),
                display_name: "a".into(),
                email: "a@b.com".into(),
                role: UserRole::Member,
            }),
            DomainEvent::UserRoleChanged(UserRoleChanged {
                user_id: 5.into(),
                old_role: UserRole::Member,
                new_role: UserRole::Admin,
            }),
            DomainEvent::UserStatusChanged(UserStatusChanged {
                user_id: 5.into(),
                from: UserStatus::Active,
                to: UserStatus::Suspended,
            }),
            DomainEvent::FeatureCreated(FeatureCreated {
                feature_id: 3.into(),
                slug: "f".into(),
                friendly_name: "F".into(),
                project_id: None,
            }),
            DomainEvent::FeatureStateAdvanced(FeatureStateAdvanced {
                feature_id: 3.into(),
                from: FeatureState::Created,
                to: FeatureState::Specified,
            }),
            DomainEvent::FeatureShipped(FeatureShipped {
                feature_id: 3.into(),
                slug: "f".into(),
            }),
            DomainEvent::WorkPackageCreated(WorkPackageCreated {
                wp_id: 20.into(),
                feature_id: 3.into(),
                title: "w".into(),
                sequence: 1,
            }),
            DomainEvent::WorkPackageStateChanged(WorkPackageStateChanged {
                wp_id: 20.into(),
                feature_id: 3.into(),
                from: WpState::Planned,
                to: WpState::Doing,
            }),
        ];

        let mut seen = HashSet::new();
        for event in events {
            let env = envelope(1, event);
            let subject = derive_subject("agileplus", &env);
            assert!(seen.insert(subject.clone()), "duplicate subject: {subject}");
        }
        assert_eq!(seen.len(), 16, "expected 16 unique subjects");
    }

    // ── envelope serialization round-trip ─────────────────────────────────────

    #[test]
    fn envelope_to_bytes_round_trip_project_created() {
        let env = envelope(
            1,
            DomainEvent::ProjectCreated(ProjectCreated {
                project_id: 1.into(),
                slug: "my-project".into(),
                name: "My Project".into(),
            }),
        );
        let bytes = envelope_to_bytes(&env).expect("serialize");
        let decoded = envelope_from_bytes(&bytes).expect("deserialize");

        assert_eq!(env.id, decoded.id);
        assert_eq!(env.aggregate_type, decoded.aggregate_type);
        assert_eq!(decoded.payload.event_type(), "project.created");
    }

    #[test]
    fn envelope_to_bytes_round_trip_feature_state_advanced() {
        let env = envelope(
            3,
            DomainEvent::FeatureStateAdvanced(FeatureStateAdvanced {
                feature_id: 3.into(),
                from: FeatureState::Created,
                to: FeatureState::Specified,
            }),
        );
        let bytes = envelope_to_bytes(&env).expect("serialize");
        let decoded = envelope_from_bytes(&bytes).expect("deserialize");

        assert_eq!(env.id, decoded.id);
        assert_eq!(decoded.aggregate_type, "Feature");
        match decoded.payload {
            DomainEvent::FeatureStateAdvanced(f) => {
                assert_eq!(f.from, FeatureState::Created);
                assert_eq!(f.to, FeatureState::Specified);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn envelope_to_bytes_round_trip_work_package_state_changed() {
        let env = envelope(
            20,
            DomainEvent::WorkPackageStateChanged(WorkPackageStateChanged {
                wp_id: 20.into(),
                feature_id: 3.into(),
                from: WpState::Planned,
                to: WpState::Doing,
            }),
        );
        let bytes = envelope_to_bytes(&env).expect("serialize");
        let decoded = envelope_from_bytes(&bytes).expect("deserialize");
        assert_eq!(decoded.aggregate_type, "WorkPackage");
        assert_eq!(decoded.payload.event_type(), "work_package.state_changed");
    }

    #[test]
    fn subject_and_bytes_are_consistent() {
        // The subject derived before serialization must match what is re-derived
        // after deserialization (idempotency check).
        let env = envelope(
            5,
            DomainEvent::UserAdded(UserAdded {
                user_id: 5.into(),
                display_name: "Bob".into(),
                email: "bob@example.com".into(),
                role: UserRole::Admin,
            }),
        );
        let subject_before = derive_subject("agileplus", &env);
        let bytes = envelope_to_bytes(&env).unwrap();
        let decoded = envelope_from_bytes(&bytes).unwrap();
        let subject_after = derive_subject("agileplus", &decoded);
        assert_eq!(subject_before, subject_after);
    }

    // ── integration (broker required — gated) ─────────────────────────────────

    /// Verifies that `NatsEventBus::connect` + `publish` work against a live
    /// broker.  Run with:
    ///   NATS_URL=nats://localhost:4222 cargo test -p agileplus-nats -- --ignored
    #[tokio::test]
    #[ignore = "requires live NATS broker"]
    async fn integration_publish_project_created() {
        let url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".into());
        let config = NatsConfig::new(url);
        let bus = NatsEventBus::connect(&config)
            .await
            .expect("connect to NATS");

        let env = envelope(
            1,
            DomainEvent::ProjectCreated(ProjectCreated {
                project_id: 1.into(),
                slug: "integration-test".into(),
                name: "Integration Test".into(),
            }),
        );

        bus.publish(env).await.expect("publish should succeed");
    }
}
