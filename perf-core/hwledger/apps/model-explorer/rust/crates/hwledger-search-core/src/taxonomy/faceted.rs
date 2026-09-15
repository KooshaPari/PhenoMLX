//! `Facets` — the structured-query side of search.
//!
//! A `Facets` value is constructed by callers (CLI, server, MCP) to describe
//! what they want; the index layer applies the structured filter alongside
//! the BM25 / semantic scorers.

use serde::{Deserialize, Serialize};

use super::arch::{ArchKind, AttentionKind};
use super::modality::Modality;
use super::model_kind::ModelKind;

/// Structured filters applied to every search query.
///
/// All collections are interpreted as **OR**, i.e. `kinds: [Base, Coding]`
/// matches models whose kind is Base *or* Coding. Numeric ranges are
/// inclusive on both ends. `Option`-typed scalars mean "no constraint".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Facets {
    /// Restrict by `ModelKind`.
    #[serde(default)]
    pub kinds: Vec<ModelKind>,

    /// Restrict by input/output modality.
    #[serde(default)]
    pub modalities: Vec<Modality>,

    /// Restrict by overall token-mixing strategy.
    #[serde(default)]
    pub arch_kinds: Vec<ArchKind>,

    /// Restrict by attention flavor.
    #[serde(default)]
    pub attention_kinds: Vec<AttentionKind>,

    /// Inclusive minimum total parameter count.
    #[serde(default)]
    pub min_param_total: Option<u64>,

    /// Inclusive maximum total parameter count.
    #[serde(default)]
    pub max_param_total: Option<u64>,

    /// Minimum `agentic_fit` score (`hwledger-search-evals`), range `[0, 1]`.
    #[serde(default)]
    pub min_agentic_fit: Option<f32>,

    /// Minimum `coding_fit` score, range `[0, 1]`.
    #[serde(default)]
    pub min_coding_fit: Option<f32>,

    /// Exact license string match (e.g. `"apache-2.0"`).
    #[serde(default)]
    pub license: Option<String>,

    /// Restrict to models that have (or do not have) benchmark evaluations.
    #[serde(default)]
    pub has_evals: Option<bool>,

    /// Restrict to models that ship at least one of these quantization tags
    /// (e.g. `["gguf", "gptq"]`).
    #[serde(default)]
    pub quants: Vec<String>,

    /// Restrict to models whose `provenance` tag equals this string
    /// (e.g. `"official"`, `"community"`).
    #[serde(default)]
    pub provenance: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let f = Facets::default();
        assert!(f.kinds.is_empty());
        assert!(f.modalities.is_empty());
        assert!(f.arch_kinds.is_empty());
        assert!(f.attention_kinds.is_empty());
        assert!(f.quants.is_empty());
        assert!(f.min_param_total.is_none());
        assert!(f.max_param_total.is_none());
        assert!(f.min_agentic_fit.is_none());
        assert!(f.min_coding_fit.is_none());
        assert!(f.license.is_none());
        assert!(f.has_evals.is_none());
        assert!(f.provenance.is_none());
    }

    #[test]
    fn round_trip() {
        let f = Facets {
            kinds: vec![ModelKind::Instruct, ModelKind::Chat],
            modalities: vec![Modality::Text, Modality::Code],
            arch_kinds: vec![ArchKind::Dense],
            attention_kinds: vec![AttentionKind::Gqa],
            min_param_total: Some(1_000_000_000),
            max_param_total: Some(70_000_000_000),
            min_agentic_fit: Some(0.5),
            min_coding_fit: Some(0.7),
            license: Some("apache-2.0".to_string()),
            has_evals: Some(true),
            quants: vec!["gguf".to_string()],
            provenance: Some("official".to_string()),
        };
        let j = serde_json::to_string(&f).unwrap();
        let back: Facets = serde_json::from_str(&j).unwrap();
        assert_eq!(back, f);
    }
}
