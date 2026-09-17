// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared application state threaded through every axum handler.
//!
//! Traceability: WP11-T069

use std::sync::Arc;

use agileplus_application::use_cases::{
    advance_feature::AdvanceFeature, create_epic::CreateEpic, create_feature::CreateFeature,
    create_story::CreateStory, transition_story::TransitionStory,
};
use agileplus_domain::config::AppConfig;
use agileplus_domain::credentials::CredentialStore;
use agileplus_domain::ports::vcs::VcsPort;
use agileplus_domain::ports::{ObservabilityPort, StoragePort};
use tokio::sync::broadcast;

/// Broadcast channel capacity for SSE event streaming.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Shared state injected into every axum handler via `State<AppState<…>>`.
pub struct AppState<S, V, O>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    pub storage: Arc<S>,
    pub vcs: Arc<V>,
    pub telemetry: Arc<O>,
    pub config: Arc<AppConfig>,
    pub credentials: Arc<dyn CredentialStore>,
    /// Broadcast sender for real-time SSE event streaming (T069).
    /// Publish JSON objects with `event_type` and `data` keys.
    pub event_tx: broadcast::Sender<serde_json::Value>,

    // ── Application use-cases (hexagonal composition root) ───────────────────
    pub create_feature_uc: Arc<CreateFeature>,
    pub advance_feature_uc: Arc<AdvanceFeature>,
    pub create_story_uc: Arc<CreateStory>,
    pub transition_story_uc: Arc<TransitionStory>,
    pub create_epic_uc: Arc<CreateEpic>,
}

impl<S, V, O> Clone for AppState<S, V, O>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            storage: Arc::clone(&self.storage),
            vcs: Arc::clone(&self.vcs),
            telemetry: Arc::clone(&self.telemetry),
            config: Arc::clone(&self.config),
            credentials: Arc::clone(&self.credentials),
            event_tx: self.event_tx.clone(),
            create_feature_uc: Arc::clone(&self.create_feature_uc),
            advance_feature_uc: Arc::clone(&self.advance_feature_uc),
            create_story_uc: Arc::clone(&self.create_story_uc),
            transition_story_uc: Arc::clone(&self.transition_story_uc),
            create_epic_uc: Arc::clone(&self.create_epic_uc),
        }
    }
}

impl<S, V, O> AppState<S, V, O>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    pub fn new(
        storage: Arc<S>,
        vcs: Arc<V>,
        telemetry: Arc<O>,
        config: Arc<AppConfig>,
        credentials: Arc<dyn CredentialStore>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self::with_event_tx(storage, vcs, telemetry, config, credentials, event_tx)
    }

    /// Create state with an explicit broadcast sender (allows sharing the channel
    /// with other subsystems such as a NATS bridge).
    pub fn with_event_tx(
        storage: Arc<S>,
        vcs: Arc<V>,
        telemetry: Arc<O>,
        config: Arc<AppConfig>,
        credentials: Arc<dyn CredentialStore>,
        event_tx: broadcast::Sender<serde_json::Value>,
    ) -> Self {
        // Composition root: wire use-cases with no-op publisher by default.
        // Production callers can swap in a NATS publisher by constructing the
        // use-cases manually before calling `with_event_tx`.
        let publisher: Arc<dyn agileplus_domain::ports::events::DomainEventPublisher> =
            Arc::new(NoOpPublisher);

        let create_feature_uc = Arc::new(CreateFeature::new(storage.clone(), publisher.clone()));
        let advance_feature_uc = Arc::new(AdvanceFeature::new(storage.clone(), publisher.clone()));
        // CreateStory / TransitionStory require a StoryRepository; StoragePort
        // also implements StoryRepository, so we can use it directly.
        let create_story_uc = Arc::new(CreateStory::new(storage.clone(), publisher.clone()));
        let transition_story_uc =
            Arc::new(TransitionStory::new(storage.clone(), publisher.clone()));
        let create_epic_uc = Arc::new(CreateEpic::new(storage.clone(), publisher.clone()));

        Self {
            storage,
            vcs,
            telemetry,
            config,
            credentials,
            event_tx,
            create_feature_uc,
            advance_feature_uc,
            create_story_uc,
            transition_story_uc,
            create_epic_uc,
        }
    }
}

// ── No-op publisher ───────────────────────────────────────────────────────────

/// Used when NATS is not configured. Events are silently dropped.
struct NoOpPublisher;

impl agileplus_domain::ports::events::DomainEventPublisher for NoOpPublisher {
    fn publish(
        &self,
        _event: agileplus_domain::ports::events::DomainEvent,
    ) -> Result<(), agileplus_domain::error::DomainError> {
        Ok(())
    }
}
