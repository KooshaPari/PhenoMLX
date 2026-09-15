//! Integration tests for `quant_tagger`.

use hwledger_search_core::RawModel;
use hwledger_search_tags::quant_tagger::tag;
use hwledger_search_tags::tager_context::TaggerContext;

fn ctx_with_entries(entries: Vec<&str>) -> TaggerContext {
    let mut raw = RawModel::new("test/test", "test");
    raw.tree_entries = entries.into_iter().map(|s| s.to_string()).collect();
    TaggerContext::from_raw(raw)
}

#[test]
fn gguf_q4_k_m_sets_gguf_and_quant() {
    let ctx = ctx_with_entries(vec!["model.Q4_K_M.gguf"]);
    let tags = tag(&ctx);
    assert!(tags.gguf_present);
    assert!(
        tags.quants.iter().any(|q| q == "q4_k_m"),
        "expected q4_k_m in {:?}",
        tags.quants
    );
}

#[test]
fn safetensors_entry_sets_safetensors_present() {
    let ctx = ctx_with_entries(vec!["model.safetensors"]);
    let tags = tag(&ctx);
    assert!(tags.safetensors_present);
}
