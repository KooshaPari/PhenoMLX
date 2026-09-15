//! Ingestion convenience — pack a model record into the Tantivy store.
//!
//! The transport shape [`IndexedModel`] is what callers (CLI ingesters, the
//! `hwledger-search-ingest` crate, MCP server) construct before handing it to
//! [`upsert_model`]. The store-specific field slicing happens here so the
//! upstream callers don't have to know how the schema lays out names.

use serde::{Deserialize, Serialize};

use crate::error::IndexError;
use crate::tantivy_store::{IndexedDoc, TantivyStore};

/// One row to be indexed. Mirrors a typical model card: id, name, org, kind,
/// family, arch, the list of quantization formats it ships, and a short
/// snippet of the model card body for free-text recall.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexedModel {
    /// Stable primary key, e.g. `"qwen/Qwen2.5-7B-Instruct"` or a HuggingFace
    /// revision id.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Owner / publisher (e.g. `"qwen"`, `"meta-llama"`).
    pub org: String,
    /// Coarse model kind (e.g. `"instruct"`, `"base"`). We store this as
    /// a tokenized text field so callers can filter by exact keyword.
    pub kind: String,
    /// Architecture family (e.g. `"qwen2"`, `"llama"`).
    pub family: String,
    /// Attention block flavor (e.g. `"gqa"`, `"mha"`).
    pub arch: String,
    /// Quantization formats the model ships in. We join with spaces so the
    /// default token indexer treats each token separately.
    pub quants: Vec<String>,
    /// First ~2000 chars of the model card body, for free-text recall.
    pub card_snippet: String,
}

impl IndexedModel {
    /// Builder helper: returns `self` with `card_snippet` truncated to the
    /// first 2000 chars (as tantivy's default token stream handles anything
    /// beyond that poorly as a single field anyway).
    ///
    /// Truncation is done at a UTF-8 char boundary so multi-byte chars
    /// (CJK, emoji, accented Latin) never panic on `is_char_boundary`.
    #[must_use]
    pub fn truncated(mut self) -> Self {
        const MAX: usize = 2000;
        if self.card_snippet.len() > MAX {
            // Find the largest char boundary ≤ MAX. `floor_char_boundary`
            // is a safe O(n) scan for non-ASCII content; the 2000-char cap
            // makes this a constant-time operation in practice.
            let mut cut = MAX;
            while cut > 0 && !self.card_snippet.is_char_boundary(cut) {
                cut -= 1;
            }
            self.card_snippet.truncate(cut);
        }
        self
    }
}

/// Insert or replace `model` in `store`.
///
/// This is a thin wrapper over [`TantivyStore::upsert`]; the wrapper exists
/// so callers don't have to remember to join the `quants` array with spaces
/// before handing it off. Internally it constructs a borrowed
/// [`IndexedDoc`] from the owned [`IndexedModel`] — no per-field
/// re-allocation is needed beyond joining the quant list.
pub fn upsert_model(store: &TantivyStore, model: &IndexedModel) -> Result<(), IndexError> {
    if model.id.is_empty() {
        return Err(IndexError::InvalidArgs("model.id is empty".into()));
    }
    let quants_joined = model.quants.join(" ");
    let doc = IndexedDoc {
        id: &model.id,
        name: &model.name,
        org: &model.org,
        kind: &model.kind,
        family: &model.family,
        arch: &model.arch,
        quants: &quants_joined,
        card_snippet: &model.card_snippet,
    };
    store.upsert(&doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncated_short_cards_are_unchanged() {
        let m = IndexedModel {
            id: "x".into(),
            name: "x".into(),
            org: "o".into(),
            kind: "instruct".into(),
            family: "f".into(),
            arch: "gqa".into(),
            quants: vec!["gguf".into()],
            card_snippet: "hello".into(),
        };
        let m = m.truncated();
        assert_eq!(m.card_snippet, "hello");
    }

    #[test]
    fn truncated_long_cards_are_cut() {
        let long = "a".repeat(3000);
        let m = IndexedModel {
            id: "x".into(),
            name: "x".into(),
            org: "o".into(),
            kind: "instruct".into(),
            family: "f".into(),
            arch: "gqa".into(),
            quants: vec!["gguf".into()],
            card_snippet: long,
        };
        assert_eq!(m.truncated().card_snippet.len(), 2000);
    }

    #[test]
    fn truncated_handles_multibyte_utf8() {
        // Real HF cards include CJK / emoji / accented text. 2000 chars
        // of multi-byte content must not panic at char boundary.
        let multibyte: String = "🦀".repeat(1500) + "中文".repeat(500).as_str();
        let m = IndexedModel {
            id: "x".into(),
            name: "x".into(),
            org: "o".into(),
            kind: "instruct".into(),
            family: "f".into(),
            arch: "gqa".into(),
            quants: vec!["gguf".into()],
            card_snippet: multibyte,
        };
        let truncated = m.truncated().card_snippet;
        assert!(truncated.len() <= 2000);
        assert!(truncated.is_char_boundary(truncated.len()));
    }

    #[test]
    fn upsert_rejects_empty_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = TantivyStore::open(dir.path()).unwrap();
        let m = IndexedModel {
            id: String::new(),
            name: "x".into(),
            org: "o".into(),
            kind: "instruct".into(),
            family: "f".into(),
            arch: "gqa".into(),
            quants: vec![],
            card_snippet: String::new(),
        };
        assert!(matches!(
            upsert_model(&store, &m),
            Err(IndexError::InvalidArgs(_))
        ));
    }
}