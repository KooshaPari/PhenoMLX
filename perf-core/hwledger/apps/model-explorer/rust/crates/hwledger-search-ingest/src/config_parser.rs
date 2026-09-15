//! Lightweight HuggingFace `config.json` extractor.
//!
//! The full `config.json` payload is enormous and family-specific. We do
//! not try to model it — instead we pull out the dozen-or-so fields that
//! drive downstream taxonomy/skill inference and leave the rest available
//! to callers through the raw `RawModel::config_json` blob.

use serde::{Deserialize, Serialize};

/// Architectural / dimensional fields extracted from a `config.json` blob.
///
/// All fields are `Option<_>` so the parser tolerates missing keys —
/// different model families publish wildly different schemas (e.g.
/// `num_local_experts` vs `num_experts`, `n_layer` vs `num_hidden_layers`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParsedConfig {
    /// `architectures` array as published by HF (e.g. `["Qwen2ForCausalLM"]`).
    pub architectures: Vec<String>,
    /// `model_type` (e.g. `"qwen2"`, `"llama"`, `"mixtral"`).
    pub model_type: Option<String>,
    /// Total number of experts (MoE models only).
    pub num_experts: Option<u32>,
    /// Number of experts active per token (MoE models only).
    pub active_experts: Option<u32>,
    /// `hidden_size` / `d_model`.
    pub hidden_size: Option<u32>,
    /// `num_hidden_layers` / `n_layer`.
    pub num_layers: Option<u32>,
    /// `intermediate_size`.
    pub intermediate_size: Option<u32>,
    /// `vocab_size`.
    pub vocab_size: Option<u32>,
    /// `max_position_embeddings`.
    pub max_position_embeddings: Option<u32>,
    /// `rope_theta` (RoPE base frequency).
    pub rope_theta: Option<f32>,
    /// `rope_scaling` object — passed through verbatim because its shape
    /// changes per family (linear, dynamic, yarn, …).
    pub rope_scaling: Option<serde_json::Value>,
}

/// Parse a HF `config.json` blob into a [`ParsedConfig`].
///
/// Missing fields become `None`; non-array `architectures` is coerced into
/// a single-element vector when possible.
pub fn parse_config_value(v: &serde_json::Value) -> ParsedConfig {
    ParsedConfig {
        architectures: extract_architectures(v),
        model_type: extract_str(v, &["model_type"]),
        num_experts: extract_u32(v, &["num_experts", "num_local_experts", "n_routed_experts"]),
        active_experts: extract_u32(
            v,
            &["num_experts_per_tok", "num_active_experts", "experts_per_token"],
        ),
        hidden_size: extract_u32(v, &["hidden_size", "d_model", "n_embd"]),
        num_layers: extract_u32(v, &["num_hidden_layers", "n_layer", "num_layers"]),
        intermediate_size: extract_u32(v, &["intermediate_size", "ffn_dim", "n_inner"]),
        vocab_size: extract_u32(v, &["vocab_size"]),
        max_position_embeddings: extract_u32(
            v,
            &["max_position_embeddings", "max_seq_len", "n_positions"],
        ),
        rope_theta: extract_f32(v, &["rope_theta"]),
        rope_scaling: v.get("rope_scaling").cloned(),
    }
}

/// Best-effort extraction of the `architectures` array, accepting either
/// the canonical `Vec<String>` shape or a single string.
fn extract_architectures(v: &serde_json::Value) -> Vec<String> {
    match v.get("architectures") {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|i| i.as_str().map(str::to_string))
            .collect(),
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// Walk `v` looking for the first key in `keys` that resolves to a string.
fn extract_str(v: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| v.get(*k).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

/// Walk `v` looking for the first key in `keys` that resolves to a number.
fn extract_u32(v: &serde_json::Value, keys: &[&str]) -> Option<u32> {
    keys.iter()
        .find_map(|k| v.get(*k))
        .and_then(|val| match val {
            serde_json::Value::Number(n) => n.as_u64().and_then(|u| u32::try_from(u).ok()),
            serde_json::Value::String(s) => s.parse::<u32>().ok(),
            _ => None,
        })
}

/// Walk `v` looking for the first key in `keys` that resolves to a number.
fn extract_f32(v: &serde_json::Value, keys: &[&str]) -> Option<f32> {
    keys.iter()
        .find_map(|k| v.get(*k))
        .and_then(|val| match val {
            serde_json::Value::Number(n) => n.as_f64().map(|f| f as f32),
            serde_json::Value::String(s) => s.parse::<f32>().ok(),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_architectures_accepts_array_and_string() {
        let v = json!({ "architectures": ["Qwen2ForCausalLM"] });
        assert_eq!(extract_architectures(&v), vec!["Qwen2ForCausalLM"]);
        let v = json!({ "architectures": "LlamaForCausalLM" });
        assert_eq!(extract_architectures(&v), vec!["LlamaForCausalLM"]);
        let v = json!({});
        assert!(extract_architectures(&v).is_empty());
    }

    #[test]
    fn extract_u32_falls_back_to_alternate_keys() {
        let v = json!({ "n_layer": 42 });
        assert_eq!(extract_u32(&v, &["num_hidden_layers", "n_layer"]), Some(42));
        let v = json!({ "num_experts": "128" });
        assert_eq!(extract_u32(&v, &["num_experts"]), Some(128));
        let v = json!({});
        assert_eq!(extract_u32(&v, &["hidden_size"]), None);
    }
}
