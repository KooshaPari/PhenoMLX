//! `parse_config_value` for MoE Qwen3-style configs.

use hwledger_search_ingest::parse_config_value;
use serde_json::json;

#[test]
fn qwen3_30b_a3b_moe_config_extracts_expert_counts() {
    let v = json!({
        "architectures": ["Qwen3MoeForCausalLM"],
        "model_type": "qwen3_moe",
        "num_experts": 128,
        "num_experts_per_tok": 8,
        "hidden_size": 4096,
        "num_hidden_layers": 48,
        "intermediate_size": 12288,
        "vocab_size": 151936,
        "max_position_embeddings": 40960,
        "rope_theta": 1000000.0
    });
    let parsed = parse_config_value(&v);
    assert_eq!(parsed.num_experts, Some(128));
    assert_eq!(parsed.active_experts, Some(8));
    assert_eq!(parsed.num_layers, Some(48));
    assert_eq!(parsed.vocab_size, Some(151936));
}
