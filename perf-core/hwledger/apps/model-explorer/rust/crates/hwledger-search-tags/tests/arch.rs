//! Integration tests for `arch_tagger`.

use hwledger_search_core::{AttentionKind, MlpKind, RawModel};
use hwledger_search_tags::arch_tagger::tag;
use hwledger_search_tags::tager_context::TaggerContext;

/// Build a context with a `config_json` payload derived from `architectures[0]`.
fn ctx_with_architectures(arch: &str) -> TaggerContext {
    let cfg = serde_json::json!({
        "architectures": [arch],
        "model_type": arch.to_ascii_lowercase(),
    });
    let mut raw = RawModel::new("test/test", "test");
    raw.config_json = Some(cfg);
    TaggerContext::from_raw(raw)
}

#[test]
fn qwen2_returns_gqa_and_swiglu() {
    let ctx = ctx_with_architectures("Qwen2ForCausalLM");
    let tags = tag(&ctx);
    assert_eq!(tags.attention, AttentionKind::Gqa);
    assert_eq!(tags.mlp, MlpKind::SwiGlu);
}

#[test]
fn deepseek_v3_returns_mla() {
    let ctx = ctx_with_architectures("DeepseekV3ForCausalLM");
    let tags = tag(&ctx);
    assert_eq!(tags.attention, AttentionKind::Mla);
}
