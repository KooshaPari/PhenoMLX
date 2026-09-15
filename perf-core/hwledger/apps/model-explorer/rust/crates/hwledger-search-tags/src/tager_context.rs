//! Shared input context every heuristic tagger consumes.
//!
//! `TaggerContext` is the deliberately decoupled, by-value-friendly handoff
//! between the [`hwledger_search_core::SourceAdapter`] layer (which produces
//! [`hwledger_search_core::RawModel`]s) and the family of taggers in this
//! crate. Each tagger only reads the fields it cares about, so a downstream
//! caller can cheaply build a `TaggerContext` directly from a partial raw
//! model (e.g. just the `id` + `card_text`) without paying for the rest.
//!
//! The `Default` impl is `empty id / empty org / all fields `None`` which is
//! what the orchestrator relies on for the "empty context" path.

use hwledger_search_core::RawModel;

/// Input bundle handed to every heuristic tagger in `hwledger-search-tags`.
///
/// The struct is intentionally cheap to clone (a `String`, a `String`, and
/// three `Option`s that each wrap a `String` or a clone-cheap shared value)
/// so callers can pass it by reference without lifetime juggling.
#[derive(Debug, Clone, Default)]
pub struct TaggerContext {
    /// Source-native identifier (e.g. `"meta-llama/Llama-3.1-8B"`).
    ///
    /// Stored separately from `raw_model.id` so a caller can build a context
    /// without having to fabricate a full `RawModel`. When both are present,
    /// `raw_model.id` is the authoritative source.
    pub id: String,

    /// Originating organisation (e.g. `"meta-llama"`, `"mistralai"`).
    ///
    /// Used by the provenance tagger to decide whether the model is an
    /// "original" from a canonical first-party org. Empty string by default.
    pub org: String,

    /// Optional raw upstream payload (card text, config JSON, tree entries).
    pub raw_model: Option<RawModel>,

    /// Optional pipeline tag (e.g. `"text-generation"`, `"image-to-text"`).
    pub pipeline_tag: Option<String>,

    /// Optional raw README / model-card text. Useful when the caller already
    /// has it on hand and doesn't want to round-trip through `RawModel`.
    pub card_text: Option<String>,
}

impl TaggerContext {
    /// Construct a context from a `(id, org)` pair; everything else is `None`.
    pub fn from_id<I: Into<String>, O: Into<String>>(id: I, org: O) -> Self {
        Self {
            id: id.into(),
            org: org.into(),
            raw_model: None,
            pipeline_tag: None,
            card_text: None,
        }
    }

    /// Construct a context around an existing [`RawModel`].
    pub fn from_raw(raw: RawModel) -> Self {
        let org = raw
            .id
            .split('/')
            .next()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let card_text = raw.card_text.clone();
        let pipeline_tag = raw.pipeline_tag.clone();
        Self {
            id: raw.id.clone(),
            org,
            raw_model: Some(raw),
            pipeline_tag,
            card_text,
        }
    }

    /// Borrow the underlying `RawModel`, if any.
    pub fn raw(&self) -> Option<&RawModel> {
        self.raw_model.as_ref()
    }

    /// Effective source for `card_text`: prefers the inline field, falls back
    /// to the `RawModel::card_text` if present.
    pub fn effective_card_text(&self) -> Option<&str> {
        self.card_text
            .as_deref()
            .or_else(|| self.raw_model.as_ref().and_then(|r| r.card_text.as_deref()))
    }

    /// Effective `pipeline_tag`: the inline field, falling back to the raw
    /// model's `pipeline_tag`.
    pub fn effective_pipeline_tag(&self) -> Option<&str> {
        self.pipeline_tag
            .as_deref()
            .or_else(|| {
                self.raw_model
                    .as_ref()
                    .and_then(|r| r.pipeline_tag.as_deref())
            })
    }
}
