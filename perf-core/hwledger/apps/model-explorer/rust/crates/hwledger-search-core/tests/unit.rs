//! Sanity checks for taxonomy defaults.

use hwledger_search_core::{AttentionKind, Facets, ModelKind};

#[test]
fn model_kind_default_is_base() {
    assert_eq!(ModelKind::default(), ModelKind::Base);
}

#[test]
fn attention_kind_default_is_mha() {
    assert_eq!(AttentionKind::default(), AttentionKind::Mha);
}

#[test]
fn facets_default_has_empty_vecs() {
    let f = Facets::default();
    assert!(f.kinds.is_empty());
    assert!(f.modalities.is_empty());
    assert!(f.arch_kinds.is_empty());
    assert!(f.attention_kinds.is_empty());
    assert!(f.quants.is_empty());
    // No scalar constraints either.
    assert!(f.min_param_total.is_none());
    assert!(f.max_param_total.is_none());
    assert!(f.min_agentic_fit.is_none());
    assert!(f.min_coding_fit.is_none());
    assert!(f.license.is_none());
    assert!(f.has_evals.is_none());
    assert!(f.provenance.is_none());
}
