//! Pluggable upstream source abstraction.
//!
//! Every concrete data source (HF Hub, ModelScope, OCI registry, our own
//! `pheno-capacity` ledger, …) implements [`SourceAdapter`]. The rest of the
//! pipeline ingests via [`RawModel`], which is deliberately lossy: anything we
//! don't currently model is funneled into `config_json` / `tree_entries` so it
//! is at least preserved.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// A globally-unique pointer to a model across the entire mesh of sources.
///
/// `source` is the lowercase adapter name (e.g. `"hf"`, `"mscope"`,
/// `"pheno-capacity"`); `id` is whatever that adapter uses to identify the
/// artifact (typically `org/name`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CandidateId {
    /// Source identifier — matches [`SourceAdapter::name`].
    pub source: String,
    /// Source-native id (e.g. `"meta-llama/Llama-3.1-8B"`).
    pub id: String,
}

impl CandidateId {
    /// Construct a new `CandidateId`.
    pub fn new<S: Into<String>, I: Into<String>>(source: S, id: I) -> Self {
        Self {
            source: source.into(),
            id: id.into(),
        }
    }

    /// Stable string key useful for hashmap / tantivy primary keys:
    /// `"<source>::<id>"`.
    pub fn key(&self) -> String {
        format!("{}::{}", self.source, self.id)
    }
}

impl std::fmt::Display for CandidateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())
    }
}

/// Adapter-agnostic view of a model fetched from an upstream source.
///
/// Concrete adapters (HF, ModelScope, …) parse their specific payload into
/// this lossy-but-uniform representation before it crosses into the
/// `ingest → index → query` pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawModel {
    /// Source-native identifier (e.g. `"meta-llama/Llama-3.1-8B"`).
    pub id: String,
    /// The source identifier (matches [`SourceAdapter::name`]).
    pub source: String,
    /// Raw README / model card text, if available.
    pub card_text: Option<String>,
    /// Raw `config.json` / config payload as opaque JSON.
    pub config_json: Option<serde_json::Value>,
    /// Snapshot of file paths inside the repository (drives tag inference).
    pub tree_entries: Vec<String>,
    /// Total downloads counter, if upstream exposes one.
    pub downloads: Option<u64>,
    /// Total likes counter, if upstream exposes one.
    pub likes: Option<u64>,
    /// ISO-8601 timestamp of last modification, if upstream exposes one.
    pub last_modified: Option<String>,
    /// First pipeline tag observed (e.g. `"text-generation"`,
    /// `"image-to-text"`), if any.
    pub pipeline_tag: Option<String>,
}

impl RawModel {
    /// Construct a minimal record (id + source). All other fields default
    /// to "unknown".
    pub fn new<S: Into<String>>(id: S, source: S) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            card_text: None,
            config_json: None,
            tree_entries: Vec::new(),
            downloads: None,
            likes: None,
            last_modified: None,
            pipeline_tag: None,
        }
    }

    /// Canonical key suitable for use as a tantivy primary key.
    pub fn key(&self) -> String {
        format!("{}::{}", self.source, self.id)
    }

    /// Build the [`CandidateId`] that uniquely identifies this record.
    pub fn candidate_id(&self) -> CandidateId {
        CandidateId::new(self.source.clone(), self.id.clone())
    }
}

/// Adapter contract every upstream source must satisfy.
///
/// The trait is intentionally async-free: implementations can wrap
/// blocking I/O via `tokio::task::spawn_blocking` or a `reqwest::blocking`
/// handle, and adapters requiring async can be implemented as a synchronous
/// façade that drives their own runtime. Keeping this layer sync makes the
/// trait trivially `dyn`-compatible across the rest of `search-core`.
pub trait SourceAdapter: Send + Sync {
    /// Stable name of the adapter, used as the `source` field on every
    /// produced record (e.g. `"hf"`, `"mscope"`, `"pheno-capacity"`).
    fn name(&self) -> &str;

    /// Enumerate candidate ids.
    ///
    /// `query` is an optional substring/metadata filter expressed in the
    /// adapter's native syntax. Implementations may return at most `limit`
    /// rows; callers should treat the returned slice as a *hint*, not a
    /// global ranking.
    fn list_candidates(&self, query: Option<&str>, limit: usize) -> Vec<CandidateId>;

    /// Fetch the full payload for a single candidate.
    ///
    /// Returns [`CoreError::NotFound`] if the adapter cannot locate the
    /// candidate, and [`CoreError::Backend`] for any other failure.
    fn fetch_raw(&self, id: &CandidateId) -> Result<RawModel, CoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_id_key_is_stable() {
        let a = CandidateId::new("hf", "org/name");
        let b = CandidateId::new("hf", "org/name");
        let c = CandidateId::new("hf", "org/other");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.key(), "hf::org/name");
        assert_eq!(format!("{a}"), "hf::org/name");
    }

    #[test]
    fn raw_model_default_is_minimal() {
        let m = RawModel::new("foo", "hf");
        assert!(m.card_text.is_none());
        assert!(m.config_json.is_none());
        assert!(m.tree_entries.is_empty());
        assert_eq!(m.key(), "hf::foo");
        assert_eq!(m.candidate_id().source, "hf");
        assert_eq!(m.candidate_id().id, "foo");
    }

    /// Trivial adapter used to confirm the trait is dyn-compatible.
    struct _NoopAdapter;
    impl SourceAdapter for _NoopAdapter {
        fn name(&self) -> &str {
            "noop"
        }
        fn list_candidates(&self, _q: Option<&str>, _l: usize) -> Vec<CandidateId> {
            Vec::new()
        }
        fn fetch_raw(&self, id: &CandidateId) -> Result<RawModel, CoreError> {
            Ok(RawModel::new(id.id.clone(), id.source.clone()))
        }
    }

    #[test]
    fn adapter_is_dyn_compatible() {
        let a: Box<dyn SourceAdapter> = Box::new(_NoopAdapter);
        assert_eq!(a.name(), "noop");
        assert!(a.list_candidates(None, 10).is_empty());
        let raw = a
            .fetch_raw(&CandidateId::new("noop", "x"))
            .expect("noop adapter cannot fail");
        assert_eq!(raw.id, "x");
    }
}
