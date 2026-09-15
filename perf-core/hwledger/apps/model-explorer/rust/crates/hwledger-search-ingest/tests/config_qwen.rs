//! `parse_config_value` end-to-end coverage for a representative
//! `Qwen2.5` `config.json` blob.

use hwledger_search_ingest::parse_config_value;
use serde_json::json;

#[test]
fn qwen2_5_config_extracts_architectures_and_rope_theta() {
    let v = json!({
        "architectures": ["Qwen2ForCausalLM"],
        "model_type": "qwen2",
        "hidden_size": 3584,
        "num_hidden_layers": 28,
        "intermediate_size": 18944,
        "vocab_size": 152064,
        "max_position_embeddings": 131072,
        "rope_theta": 1000000.0,
        "rope_scaling": null
    });
    let parsed = parse_config_value(&v);
    assert!(
        parsed.architectures.iter().any(|a| a.contains("Qwen2")),
        "architectures must contain 'Qwen2': {:?}",
        parsed.architectures
    );
    assert_eq!(parsed.rope_theta, Some(1_000_000.0));
    assert_eq!(parsed.model_type.as_deref(), Some("qwen2"));
    assert_eq!(parsed.hidden_size, Some(3584));
    assert_eq!(parsed.num_layers, Some(28));
}

#[test]
fn missing_fields_yield_all_none() {
    let v = json!({});
    let parsed = parse_config_value(&v);
    assert!(parsed.architectures.is_empty());
    assert_eq!(parsed.model_type, None);
    assert_eq!(parsed.num_experts, None);
    assert_eq!(parsed.active_experts, None);
    assert_eq!(parsed.hidden_size, None);
    assert_eq!(parsed.num_layers, None);
    assert_eq!(parsed.intermediate_size, None);
    assert_eq!(parsed.vocab_size, None);
    assert_eq!(parsed.max_position_embeddings, None);
    assert_eq!(parsed.rope_theta, None);
    assert_eq!(parsed.rope_scaling, None);
}
