//! Integration tests for `modelkind_tagger`.

use hwledger_search_tags::modelkind_tagger::tag;
use hwledger_search_tags::tager_context::TaggerContext;

use hwledger_search_core::ModelKind;

#[test]
fn llama_instruct_id_is_chat() {
    let ctx = TaggerContext::from_id("meta-llama/Llama-3.1-8B-Instruct", "meta-llama");
    let tags = tag(&ctx);
    assert_eq!(tags.primary_kind, ModelKind::Chat);
}

#[test]
fn bge_id_is_embedding() {
    let ctx = TaggerContext::from_id("BAAI/bge-large-en-v1.5", "BAAI");
    let tags = tag(&ctx);
    assert_eq!(tags.primary_kind, ModelKind::Embedding);
}
