//! Integration tests for the top-level `tag_all` orchestrator.

use hwledger_search_core::RawModel;
use hwledger_search_tags::orchestrator::tag_all;
use hwledger_search_tags::tager_context::TaggerContext;

#[test]
fn full_qwen2_5_coder_7b_codes_well() {
    // Build a `TaggerContext` that matches the upstream Qwen2.5-Coder-7B
    // card: GQA + SwiGLU + RoPE, dense (no MoE), a GGUF artifact, and
    // a model id that hints at "coder".
    let cfg = serde_json::json!({
        "architectures": ["Qwen2ForCausalLM"],
        "model_type": "qwen2",
        "hidden_size": 3584,
        "num_hidden_layers": 28,
        "intermediate_size": 18944,
        "rope_theta": 1000000.0,
    });
    let mut raw = RawModel::new("Qwen/Qwen2.5-Coder-7B-Instruct", "hf");
    raw.config_json = Some(cfg);
    raw.tree_entries = vec!["qwen2.5-coder-7b-instruct-q5_k_m.gguf".to_string()];
    raw.card_text = Some("Qwen2.5-Coder-7B-Instruct is a code-specialized instruction-tuned model.".to_string());
    raw.pipeline_tag = Some("text-generation".to_string());
    let ctx = TaggerContext::from_raw(raw);
    let tags = tag_all(&ctx);
    assert!(
        tags.fit.coding >= 0.6,
        "coding should be >=0.6 for Qwen2.5-Coder-7B, got {}",
        tags.fit.coding
    );
}

#[test]
fn empty_context_is_default() {
    let ctx = TaggerContext::default();
    let tags = tag_all(&ctx);
    // Every structural tag axis is the documented default.
    assert!(tags.arch.arch_kind == hwledger_search_core::ArchKind::Dense);
    assert!(!tags.moe.is_moe);
    assert!(tags.quant.quants.is_empty());
    assert!(tags.kind.primary_kind == hwledger_search_core::ModelKind::Base);
    assert!(!tags.reap.is_reasoning_model);
    assert_eq!(tags.provenance.provenance, "unknown");
    assert!(tags.provenance.base_model.is_none());
    // The fit score is the only axis the formula doesn't tie to zero for
    // an empty input (it carries a small "non-MoE dense" baseline for the
    // coding axis). We assert both components are well below 0.5.
    assert!(tags.fit.agentic < 0.5);
    assert!(tags.fit.coding < 0.5);
}
