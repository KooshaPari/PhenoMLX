//! Triage adapter — high-level orchestration service for triage and backlog.
//
//! Traceability: WP17-T098b
//!
//! Wraps [`TriageClassifier`] and [`BacklogStore`] into a single service
//! that classifies free-text input, persists backlog items, and surfaces
//! triage results for gRPC, CLI, and agent dispatch layers.

use chrono::Utc;

pub use crate::backlog::{BacklogItem, BacklogStore};
pub use crate::classifier::{TriageClassifier, TriageResult};

/// A triage operation combines classification with backlog persistence.
#[derive(Debug, Clone)]
pub struct TriageOp {
    pub result: TriageResult,
    pub backlog_id: Option<i64>,
}

/// High-level triage adapter that orchestrates classification and backlog storage.
///
/// ## Responsibility
///
/// `TriageAdapter` is the single entry point for triage operations across all
/// consuming layers (CLI, gRPC, agent dispatch). It:
//
///
/// 1. Classifies free-text input via [`TriageClassifier`]
/// 2. Constructs a [`BacklogItem`] from the classification
/// 3. Persists the item via [`BacklogStore`]
/// 4. Returns a [`TriageOp`] containing the result and the backlog ID
///
/// For production use, swap `BacklogStore` with a [`StoragePort`][agileplus_domain::ports::StoragePort]
/// implementation that writes to SQLite.
#[derive(Debug)]
pub struct TriageAdapter<S = BacklogStore> {
    classifier: TriageClassifier,
    store: S,
}

impl TriageAdapter<BacklogStore> {
    /// Construct a new adapter backed by an in-memory [`BacklogStore`].
    ///
    /// Use this for CLI invocations and unit tests. For production, construct
    /// with [`TriageAdapter::with_store`] passing a [`StoragePort`] adapter.
    pub fn new() -> Self {
        Self {
            classifier: TriageClassifier::new(),
            store: BacklogStore::new(),
        }
    }
}

impl<S> TriageAdapter<S> {
    /// Construct an adapter with a custom store implementation.
    ///
    /// Accepts any type that implements the internal store interface (get / add).
    /// In practice this is [`BacklogStore`] in tests and a [`StoragePort`] in
    /// production.
    pub fn with_store(classifier: TriageClassifier, store: S) -> Self {
        Self { classifier, store }
    }

    /// Classifier reference for read-only inspection.
    pub fn classifier(&self) -> &TriageClassifier {
        &self.classifier
    }
}

impl Default for TriageAdapter<BacklogStore> {
    fn default() -> Self {
        Self::new()
    }
}

/// Source identifiers for triage events.
#[derive(Debug, Clone, Copy)]
pub enum TriageSource {
    /// Direct CLI invocation.
    Cli,
    /// Agent auto-triage during implementation.
    AgentAuto,
    /// gRPC / MCP call.
    Grpc,
    /// Manual human report.
    Human,
}

impl TriageSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::AgentAuto => "agent-auto",
            Self::Grpc => "grpc",
            Self::Human => "human",
        }
    }
}

/// Options for a triage classify-and-file operation.
#[derive(Debug, Clone, Default)]
pub struct ClassifyOptions {
    /// Optional explicit intent override (bypasses classifier).
    pub override_intent: Option<crate::classifier::Intent>,
    /// Link the backlog item to a feature slug if discovered during feature work.
    pub feature_slug: Option<String>,
    /// Tags to attach to the backlog item.
    pub tags: Vec<String>,
    /// Override the computed priority.
    pub priority: Option<crate::backlog::BacklogPriority>,
}

/// Result of a full triage classify-and-file operation.
#[derive(Debug, Clone)]
pub struct ClassifyOutcome {
    /// Classification result.
    pub result: TriageResult,
    /// Backlog item that was created (or `None` if skipped).
    pub item: BacklogItem,
    /// Backlog store ID if persisted.
    pub backlog_id: Option<i64>,
}

impl<S: super::BacklogStoreOps> TriageAdapter<S> {
    /// Classify input text and optionally persist to the backlog.
    ///
    /// Returns a [`ClassifyOutcome`] with the classification result and the
    /// created backlog item.
    ///
    /// When `options.override_intent` is set, the classifier is bypassed and
    /// the specified intent is used directly.
    ///
    /// The backlog item is always created in-memory; whether it is persisted to
    /// SQLite depends on the store implementation.
    pub fn classify(
        &mut self,
        input: &str,
        source: TriageSource,
        options: ClassifyOptions,
    ) -> ClassifyOutcome {
        let result = match options.override_intent {
            Some(intent) => self.classifier.classify_with_override(input, intent),
            None => self.classifier.classify(input),
        };

        let priority = options
            .priority
            .unwrap_or_else(|| result.intent.default_priority());

        let mut item = BacklogItem::from_triage(
            input.chars().take(120).collect::<String>(),
            input.to_string(),
            result.intent,
            source.as_str().to_string(),
        );

        item.priority = priority;
        item.feature_slug = options.feature_slug;
        item.tags = options.tags;
        item.updated_at = Utc::now();

        let backlog_id = self.store.add(item.clone());

        ClassifyOutcome {
            result,
            item,
            backlog_id: Some(backlog_id),
        }
    }

    /// Classify input without persisting (read-only classification).
    pub fn classify_readonly(&self, input: &str) -> TriageResult {
        self.classifier.classify(input)
    }

    /// List all backlog items (read-only).
    pub fn list_backlog(&self) -> Vec<BacklogItem> {
        self.store.list().to_vec()
    }

    /// List backlog items filtered by intent (read-only).
    pub fn list_by_intent(&self, intent: crate::backlog::Intent) -> Vec<BacklogItem> {
        self.store
            .list_by_intent(intent)
            .into_iter()
            .cloned()
            .collect()
    }

    /// List backlog items filtered by status (read-only).
    pub fn list_by_status(&self, status: crate::backlog::BacklogStatus) -> Vec<BacklogItem> {
        self.store
            .list_by_status(status)
            .into_iter()
            .cloned()
            .collect()
    }
}

/// Store operations required by `TriageAdapter`.
///
/// Implement this trait on any storage adapter (in-memory, SQLite, etc.)
/// to use it with `TriageAdapter::with_store`.
pub trait BacklogStoreOps {
    fn add(&mut self, item: BacklogItem) -> i64;
    fn list(&self) -> &[BacklogItem];
    fn list_by_intent(&self, intent: crate::backlog::Intent) -> Vec<&BacklogItem>;
    fn list_by_status(&self, status: crate::backlog::BacklogStatus) -> Vec<&BacklogItem>;
    fn get(&self, id: i64) -> Option<&BacklogItem>;
}

impl BacklogStoreOps for BacklogStore {
    fn add(&mut self, item: BacklogItem) -> i64 {
        BacklogStore::add(self, item)
    }

    fn list(&self) -> &[BacklogItem] {
        BacklogStore::list(self)
    }

    fn list_by_intent(&self, intent: crate::backlog::Intent) -> Vec<&BacklogItem> {
        BacklogStore::list_by_intent(self, intent)
    }

    fn list_by_status(&self, status: crate::backlog::BacklogStatus) -> Vec<&BacklogItem> {
        BacklogStore::list_by_status(self, status)
    }

    fn get(&self, id: i64) -> Option<&BacklogItem> {
        BacklogStore::get(self, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triage_adapter_classifies_and_persists() {
        let mut adapter = TriageAdapter::new();
        let outcome = adapter.classify(
            "Login button crashes on mobile Safari",
            TriageSource::Cli,
            ClassifyOptions::default(),
        );

        assert_eq!(outcome.result.intent, crate::backlog::Intent::Bug);
        assert!(outcome.result.confidence > 0.3);
        assert!(outcome.backlog_id.is_some());
        assert_eq!(outcome.item.source, "cli");
        assert_eq!(outcome.item.priority, crate::backlog::BacklogPriority::High);
    }

    #[test]
    fn triage_adapter_respects_intent_override() {
        let mut adapter = TriageAdapter::new();
        let outcome = adapter.classify(
            "What if we could export data as CSV",
            TriageSource::Cli,
            ClassifyOptions {
                override_intent: Some(crate::backlog::Intent::Feature),
                ..Default::default()
            },
        );

        assert_eq!(outcome.result.intent, crate::backlog::Intent::Feature);
        assert_eq!(outcome.result.confidence, 1.0);
    }

    #[test]
    fn triage_adapter_links_feature_slug() {
        let mut adapter = TriageAdapter::new();
        let outcome = adapter.classify(
            "Typo in welcome email",
            TriageSource::AgentAuto,
            ClassifyOptions {
                feature_slug: Some("auth-overhaul".to_string()),
                ..Default::default()
            },
        );

        assert_eq!(outcome.item.feature_slug.as_deref(), Some("auth-overhaul"));
        assert_eq!(outcome.item.source, "agent-auto");
    }

    #[test]
    fn triage_adapter_readonly_classify() {
        let adapter = TriageAdapter::new();
        let result = adapter.classify_readonly("Add dark mode support");
        assert_eq!(result.intent, crate::backlog::Intent::Feature);
    }

    #[test]
    fn triage_adapter_list_backlog() {
        let mut adapter = TriageAdapter::new();
        adapter.classify("bug 1", TriageSource::Cli, ClassifyOptions::default());
        adapter.classify("bug 2", TriageSource::Cli, ClassifyOptions::default());
        adapter.classify(
            "great idea",
            TriageSource::Cli,
            ClassifyOptions {
                override_intent: Some(crate::backlog::Intent::Idea),
                ..Default::default()
            },
        );

        let all = adapter.list_backlog();
        assert_eq!(all.len(), 3);

        let bugs = adapter.list_by_intent(crate::backlog::Intent::Bug);
        assert_eq!(bugs.len(), 2);

        let new_items = adapter.list_by_status(crate::backlog::BacklogStatus::New);
        assert_eq!(new_items.len(), 3);
    }

    #[test]
    fn triage_adapter_get_backlog_item() {
        let mut adapter = TriageAdapter::new();
        let outcome = adapter.classify(
            "Out of memory error",
            TriageSource::Cli,
            ClassifyOptions::default(),
        );
        let id = outcome.backlog_id.unwrap();

        let item = adapter.store.get(id);
        assert!(item.is_some());
        assert_eq!(item.unwrap().title, "Out of memory error");
    }

    #[test]
    fn triage_adapter_default_source_is_cli() {
        let mut adapter = TriageAdapter::new();
        let outcome = adapter.classify("error", TriageSource::Cli, ClassifyOptions::default());
        assert_eq!(outcome.item.source, "cli");
    }

    #[test]
    fn triage_adapter_respects_priority_override() {
        let mut adapter = TriageAdapter::new();
        let outcome = adapter.classify(
            "login broken",
            TriageSource::Cli,
            ClassifyOptions {
                priority: Some(crate::backlog::BacklogPriority::Critical),
                ..Default::default()
            },
        );

        assert_eq!(
            outcome.item.priority,
            crate::backlog::BacklogPriority::Critical
        );
    }

    #[test]
    fn triage_adapter_with_tags() {
        let mut adapter = TriageAdapter::new();
        let outcome = adapter.classify(
            "Memory leak in worker",
            TriageSource::Cli,
            ClassifyOptions {
                tags: vec!["performance".to_string(), "workers".to_string()],
                ..Default::default()
            },
        );

        assert_eq!(outcome.item.tags, vec!["performance", "workers"]);
    }
}
