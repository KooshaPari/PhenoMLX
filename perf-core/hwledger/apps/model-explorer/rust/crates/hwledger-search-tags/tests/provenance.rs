//! Integration tests for `provenance_tagger`.

use hwledger_search_tags::provenance_tagger::tag;
use hwledger_search_tags::tager_context::TaggerContext;

#[test]
fn meta_llama_id_is_original() {
    let ctx = TaggerContext::from_id("meta-llama/Llama-3.1-8B", "meta-llama");
    let tags = tag(&ctx);
    assert_eq!(tags.provenance, "original");
    assert_eq!(tags.base_model.as_deref(), Some("meta-llama/Llama-3.1-8B"));
}
