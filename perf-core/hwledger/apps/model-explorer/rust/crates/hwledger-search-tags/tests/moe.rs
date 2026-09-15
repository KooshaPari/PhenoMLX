//! Integration tests for `moe_tagger`.

use hwledger_search_core::RawModel;
use hwledger_search_tags::moe_tagger::tag;
use hwledger_search_tags::tager_context::TaggerContext;

#[test]
fn config_with_num_experts_is_moe() {
    let cfg = serde_json::json!({
        "num_experts": 8,
        "num_experts_per_tok": 2,
    });
    let mut raw = RawModel::new("test/mixtral-style", "test");
    raw.config_json = Some(cfg);
    let ctx = TaggerContext::from_raw(raw);
    let tags = tag(&ctx);
    assert!(tags.is_moe);
    assert_eq!(tags.num_experts, Some(8));
    assert_eq!(tags.active_experts, Some(2));
}
