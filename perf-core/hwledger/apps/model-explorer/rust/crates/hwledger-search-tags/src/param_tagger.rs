//! Parameter tagger.
//!
//! Combines an upstream `num_parameters` (if exposed) with a coarse
//! transformer-flops back-of-envelope estimator to produce both an exact
//! and a bucket-friendly total parameter count, plus a string bucket
//! (`"<1B"`, `"1B-7B"`, …, `"70B+"`) used by [`Facets`].
//!
//! [`Facets`]: hwledger_search_core::Facets

use crate::moe_tagger::MoeTags;
use crate::tager_context::TaggerContext;

/// Coarse parameter bucket for faceted search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamTags {
    /// Total parameter count for the model, if available.
    pub total_params: Option<u64>,
    /// Active parameter count (== total for a dense model). For MoE models
    /// we propagate `MoeTags::active_params` if the upstream didn't supply
    /// its own.
    pub active_params: Option<u64>,
    /// Bucket label, e.g. `"1B-7B"`, `"70B+"`. `None` only when no
    /// upstream signal is available at all.
    pub bucket: Option<String>,
}

impl Default for ParamTags {
    fn default() -> Self {
        Self {
            total_params: None,
            active_params: None,
            bucket: None,
        }
    }
}

/// Estimate the model size from a single config JSON.
///
/// Reads `num_parameters` first if exposed; otherwise falls back to a rough
/// transformer-shape estimate:
/// `hidden_size * num_hidden_layers * (intermediate_size + hidden_size) * 12`
/// (the canonical 12x accounts for Q/K/V/O projections + 2 MLP matrices
/// + embeddings + LM head, give or take — accurate to ~25%).
pub fn tag(ctx: &TaggerContext, moe: &MoeTags) -> ParamTags {
    let cfg = ctx.raw().and_then(|r| r.config_json.as_ref());

    let total_params = cfg
        .and_then(|c| c.get("num_parameters"))
        .and_then(|v| v.as_u64())
        .or_else(|| estimate_from_config(cfg));

    let active_params = if moe.is_moe {
        moe.active_params
            .or(total_params)
    } else {
        total_params
    };

    let bucket = total_params.and_then(bucket_for);

    ParamTags {
        total_params,
        active_params,
        bucket,
    }
}

/// Rough transformer shape estimator.
///
/// Returns `None` if any of `hidden_size`, `num_hidden_layers`, or
/// `intermediate_size` is missing — the caller falls back to no-data.
fn estimate_from_config(cfg: Option<&serde_json::Value>) -> Option<u64> {
    let c = cfg?;
    let hidden = c.get("hidden_size")?.as_u64()?;
    let layers = c.get("num_hidden_layers")?.as_u64()?;
    let inter = c.get("intermediate_size")?.as_u64()?;
    Some(hidden.saturating_mul(layers).saturating_mul(inter + hidden) * 12)
}

/// Map a raw `total_params` to one of the documented bucket strings.
fn bucket_for(p: u64) -> Option<String> {
    // Bucket edges in absolute parameter counts.
    const ONE_B: u64 = 1_000_000_000;
    const SEVEN_B: u64 = 7_000_000_000;
    const THIRTEEN_B: u64 = 13_000_000_000;
    const THIRTYFIVE_B: u64 = 35_000_000_000;
    const SEVENTY_B: u64 = 70_000_000_000;

    // <=1B → "<=1B" (note the <= form to match downstream facet vocabulary).
    if p <= ONE_B {
        return Some("<=1B".to_string());
    }
    if p <= SEVEN_B {
        return Some("1B-7B".to_string());
    }
    if p <= THIRTEEN_B {
        return Some("7B-13B".to_string());
    }
    if p <= THIRTYFIVE_B {
        return Some("13B-35B".to_string());
    }
    if p <= SEVENTY_B {
        return Some("35B-70B".to_string());
    }
    Some("70B+".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_edges() {
        assert_eq!(bucket_for(1).as_deref(), Some("<=1B"));
        assert_eq!(bucket_for(1_000_000_000).as_deref(), Some("<=1B"));
        assert_eq!(bucket_for(1_500_000_000).as_deref(), Some("1B-7B"));
        assert_eq!(bucket_for(7_000_000_000).as_deref(), Some("1B-7B"));
        assert_eq!(bucket_for(7_500_000_000).as_deref(), Some("7B-13B"));
        assert_eq!(bucket_for(13_000_000_000).as_deref(), Some("7B-13B"));
        assert_eq!(bucket_for(13_500_000_000).as_deref(), Some("13B-35B"));
        assert_eq!(bucket_for(35_000_000_000).as_deref(), Some("13B-35B"));
        assert_eq!(bucket_for(35_500_000_000).as_deref(), Some("35B-70B"));
        assert_eq!(bucket_for(70_000_000_000).as_deref(), Some("35B-70B"));
        assert_eq!(bucket_for(70_500_000_000).as_deref(), Some("70B+"));
    }
}
