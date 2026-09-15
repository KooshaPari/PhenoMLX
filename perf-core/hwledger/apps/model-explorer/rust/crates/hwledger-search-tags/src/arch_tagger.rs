//! Architecture-family tagger.
//!
//! Maps a model's upstream `config.json` (and, where present, the
//! `architectures[]` / `model_type` strings) to the four core architecture
//! axes exposed by `hwledger-search-core`:
//!
//! * [`ArchKind`] — overall token-mixing strategy (`Dense`, `MoE`, `Hybrid`).
//! * [`AttentionKind`] — attention flavor (`Mha`, `Gqa`, `Mla`, `Ssm`, …).
//! * [`MlpKind`] — MLP block flavor (`Standard`, `SwiGlu`, `GeLu`).
//! * [`RopeVariant`] — RoPE variant (`None`, `Standard`, `Llama3`, …).
//!
//! The mapping is entirely heuristic: it pattern-matches on the family name
//! encoded in `architectures[0]` / `model_type` and does not consume the
//! config-derived truth (e.g. `num_key_value_heads`). That's a future
//! refinement once we start round-tripping against HF weights.
//!
//! The default value of every field is the default of the underlying enum
//! (`ArchKind::Dense`, `AttentionKind::Mha`, `MlpKind::Standard`,
//! `RopeVariant::None`).

use hwledger_search_core::{ArchKind, AttentionKind, MlpKind, RopeVariant};
use serde_json::Value;

use crate::tager_context::TaggerContext;

/// The four architecture axes for a single model.
///
/// `Default` produces the "unknown / dense vanilla transformer" profile
/// (`Dense`, `Mha`, `Standard`, `None`), which is also what every tagger
/// returns when no upstream signal is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArchTags {
    /// Overall token-mixing strategy.
    pub arch_kind: ArchKind,
    /// Attention flavor.
    pub attention: AttentionKind,
    /// MLP block flavor.
    pub mlp: MlpKind,
    /// RoPE positional-encoding variant.
    pub rope: RopeVariant,
}

impl Default for ArchTags {
    fn default() -> Self {
        Self {
            arch_kind: ArchKind::default(),
            attention: AttentionKind::default(),
            mlp: MlpKind::default(),
            rope: RopeVariant::default(),
        }
    }
}

/// Heuristic pattern-match a [`TaggerContext`] into [`ArchTags`].
///
/// Inspects (in order):
/// 1. `architectures[0]` if present.
/// 2. `model_type` if present.
/// 3. `id` / `org` fallbacks when no config_json is available.
///
/// The mapping is intentionally coarse — it commits to family-level claims
/// only. Variants within a family (e.g. Llama-3 vs Llama-2) are visualised
/// against the same `attention` / `mlp` defaults and only get a custom
/// `rope` variant if the model is past Llama 3.1.
pub fn tag(ctx: &TaggerContext) -> ArchTags {
    let raw = ctx.raw();
    let cfg: Option<&Value> = raw.and_then(|r| r.config_json.as_ref());

    // 1. Pull the family name from config_json — first the `architectures`
    // array, then `model_type`. Both fields are conventional on HF.
    let family = cfg
        .and_then(|c| c.get("architectures"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_ascii_lowercase())
        .or_else(|| {
            cfg.and_then(|c| c.get("model_type"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_ascii_lowercase())
        })
        // 2. Last-resort fallback: scan the id / org.
        .unwrap_or_else(|| {
            let hay = format!("{} {}", ctx.id, ctx.org).to_ascii_lowercase();
            if hay.is_empty() {
                String::new()
            } else {
                hay
            }
        });

    let mut arch_kind = derive_arch_kind(&family, cfg);
    let mut attention = AttentionKind::default();
    let mut mlp = MlpKind::default();
    let mut rope = RopeVariant::default();

    // Family-level dispatch. Order matters: more-specific patterns first.
    if family.contains("qwen3") && (family.contains("moe") || has_moe_keys(cfg)) {
        // Qwen3-MoE + DeepSeek-V3 family use MLA.
        arch_kind = ArchKind::Moe;
        attention = AttentionKind::Mla;
        mlp = MlpKind::SwiGlu;
        rope = RopeVariant::Standard;
    } else if family.contains("qwen") {
        attention = AttentionKind::Gqa;
        mlp = MlpKind::SwiGlu;
        rope = RopeVariant::Standard;
    } else if family.contains("llama") {
        attention = AttentionKind::Gqa;
        mlp = MlpKind::SwiGlu;
        rope = if is_llama3(&family, cfg) {
            RopeVariant::Llama3
        } else {
            RopeVariant::Standard
        };
    } else if family.contains("mistral") || family.contains("mixtral") {
        attention = AttentionKind::Gqa;
        mlp = MlpKind::SwiGlu;
        rope = RopeVariant::Standard;
        if family.contains("mixtral") || has_moe_keys(cfg) {
            arch_kind = ArchKind::Moe;
        }
    } else if family.contains("gemma") {
        attention = AttentionKind::Gqa;
        mlp = MlpKind::GeLu;
        rope = RopeVariant::Standard;
    } else if family.contains("deepseek_v2")
        || family.contains("deepseek_v3")
        || family.contains("deepseek-v2")
        || family.contains("deepseek-v3")
        || family.contains("deepseekv2")
        || family.contains("deepseekv3")
    {
        arch_kind = ArchKind::Moe;
        attention = AttentionKind::Mla;
        mlp = MlpKind::SwiGlu;
        rope = RopeVariant::Standard;
    } else if family.contains("mamba") || family.contains("rwkv") {
        attention = AttentionKind::Ssm;
        mlp = MlpKind::Standard;
        rope = RopeVariant::None;
    } else if family.contains("jamba") {
        arch_kind = ArchKind::Hybrid;
        attention = AttentionKind::Hybrid;
        mlp = MlpKind::SwiGlu;
        rope = RopeVariant::Standard;
    } else if family.contains("phi3") || family.contains("phi-3") || family.contains("phi_3") {
        attention = AttentionKind::Mqa;
        mlp = MlpKind::SwiGlu;
        rope = RopeVariant::Standard;
    } else if family.is_empty() {
        // No signal at all: keep defaults.
    } else {
        // Known family not enumerated above: fall back to MHA + Standard.
        attention = AttentionKind::Mha;
        mlp = MlpKind::Standard;
        rope = RopeVariant::None;
    }

    ArchTags {
        arch_kind,
        attention,
        mlp,
        rope,
    }
}

/// Read `architectures[0]` and decide whether the model is `MoE`, `Hybrid`,
/// or `Dense`. Falls back to inspecting `num_local_experts` / `n_routed_experts`
/// when the architectures string isn't distinguishing.
fn derive_arch_kind(family: &str, cfg: Option<&Value>) -> ArchKind {
    if family.contains("moe") || family.contains("sparse") || family.contains("mixtral") {
        return ArchKind::Moe;
    }
    if family.contains("hybrid") || family.contains("jamba") {
        return ArchKind::Hybrid;
    }
    if has_moe_keys(cfg) {
        return ArchKind::Moe;
    }
    ArchKind::Dense
}

/// `true` if the config_json looks like a MoE config (any of the well-known
/// expert-count keys being present).
fn has_moe_keys(cfg: Option<&Value>) -> bool {
    let Some(c) = cfg else {
        return false;
    };
    for key in [
        "num_local_experts",
        "num_experts",
        "n_routed_experts",
        "num_selected_experts",
        "moe_num_experts",
    ] {
        if c.get(key).is_some() {
            return true;
        }
    }
    false
}

/// Heuristic: is this a Llama-3.x family member? We look at the family
/// string for `3.1` / `3.2` / `3.3` markers, and also probe `rope_theta`
/// (Llama-3 bumped it to 500_000).
fn is_llama3(family: &str, cfg: Option<&Value>) -> bool {
    // Family string: "LlamaForCausalLM" + .id tail "Llama-3.1-…" etc.
    if family.contains("3.1") || family.contains("3.2") || family.contains("3.3") {
        return true;
    }
    if let Some(c) = cfg {
        if let Some(theta) = c.get("rope_theta").and_then(|v| v.as_f64()) {
            if theta >= 500_000.0 {
                return true;
            }
        }
    }
    false
}
