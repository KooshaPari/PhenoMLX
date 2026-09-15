//! MoE (mixture-of-experts) tagger.
//!
//! Reads the conventional HF / DeepSeek expert-count keys from
//! `config_json` and exposes the (optional) totals for downstream scoring
//! and faceting.

use crate::tager_context::TaggerContext;

/// Aggregate of the MoE-related fields we discovered in a `config_json`.
///
/// `Default` is `is_moe = false` and every numeric field is `None`, which is
/// the expected shape for a non-MoE model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoeTags {
    /// `true` if the model is a confirmed mixture-of-experts.
    pub is_moe: bool,
    /// Total number of experts the router can choose from.
    pub num_experts: Option<u32>,
    /// Number of experts activated per token (i.e. top-k).
    pub active_experts: Option<u32>,
    /// Total parameter count, if upstream exposes it.
    pub total_params: Option<u64>,
    /// Active parameter count inferred as `total_params * (active/num)`.
    pub active_params: Option<u64>,
}

impl Default for MoeTags {
    fn default() -> Self {
        Self {
            is_moe: false,
            num_experts: None,
            active_experts: None,
            total_params: None,
            active_params: None,
        }
    }
}

/// Heuristically tag a [`TaggerContext`] as MoE or not.
pub fn tag(ctx: &TaggerContext) -> MoeTags {
    let cfg = ctx.raw().and_then(|r| r.config_json.as_ref());

    let Some(cfg) = cfg else {
        return MoeTags::default();
    };

    // Probe the well-known expert-count keys. The first one to yield a u32
    // wins; everything else is then computed consistently.
    let num_experts = read_u32_any(
        cfg,
        &[
            "num_local_experts",
            "num_experts",
            "n_routed_experts",
            "moe_num_experts",
            "num_experts_per_layer",
        ],
    );

    let Some(num_experts) = num_experts else {
        return MoeTags::default();
    };

    let active_experts = read_u32_any(
        cfg,
        &[
            "num_experts_per_tok",
            "num_selected_experts",
            "top_k",
            "moe_top_k",
            "num_active_experts",
        ],
    );

    // num_parameters is the conventional HF field for total params.
    let total_params = read_u64_any(cfg, &["num_parameters", "total_params"]);

    // If we know both the active count and the total, derive active_params
    // as `total * (active / num)`. We use rounding so the e.g. 4/8 ratio
    // doesn't accidentally shave off a single param.
    let active_params = match (total_params, active_experts) {
        (Some(t), Some(a)) => {
            let a = a as u64;
            let n = num_experts as u64;
            if n == 0 {
                None
            } else {
                Some((t as f64 * (a as f64 / n as f64)).round() as u64)
            }
        }
        _ => None,
    };

    MoeTags {
        is_moe: true,
        num_experts: Some(num_experts),
        active_experts,
        total_params,
        active_params,
    }
}

/// Read the first key in `keys` that yields a `u32` from `cfg`.
fn read_u32_any(cfg: &serde_json::Value, keys: &[&str]) -> Option<u32> {
    for k in keys {
        if let Some(v) = cfg.get(*k) {
            if let Some(n) = v.as_u64() {
                return u32::try_from(n).ok();
            }
        }
    }
    None
}

/// Read the first key in `keys` that yields a `u64` from `cfg`.
fn read_u64_any(cfg: &serde_json::Value, keys: &[&str]) -> Option<u64> {
    for k in keys {
        if let Some(v) = cfg.get(*k) {
            if let Some(n) = v.as_u64() {
                return Some(n);
            }
        }
    }
    None
}
