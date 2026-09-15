//! Neighborhood expansion.
//!
//! v1 stub: returns the seed ids unchanged and emits a `tracing::info!`
//! so operators can correlate seed-vs-expansion traffic in production.
//! The real implementation lands when neighborhood discovery — fetching
//! the "used by" / "forks of" graphs from the upstream source — is in
//! scope. Hooks for it are already wired into the public surface so
//! downstream callers can compose without churn.

use hwledger_search_core::SourceAdapter;

use crate::lazy_populate::PopulateGate;

/// Tuning knobs for neighborhood expansion.
#[derive(Debug, Clone)]
pub struct ExpansionConfig {
    /// Upper bound on how many *new* neighbors to surface per seed.
    /// v1 ignores this; preserved so the parameter list is stable.
    pub max_neighbors: usize,
}

impl Default for ExpansionConfig {
    fn default() -> Self {
        Self {
            max_neighbors: 10,
        }
    }
}

/// v1 stub. Returns `seed_ids` unchanged. The adapter and gate are
/// accepted but unused — the function signature is the contract
/// downstream already depends on.
pub fn expand_neighborhood<A>(
    _adapter: &A,
    _gate: &PopulateGate,
    seed_ids: Vec<String>,
) -> Vec<String>
where
    A: SourceAdapter + ?Sized,
{
    tracing::info!(
        seed_count = seed_ids.len(),
        "expansion deferred to lazy populate + neighborhood crawl"
    );
    seed_ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion_config_default_is_ten() {
        let c = ExpansionConfig::default();
        assert_eq!(c.max_neighbors, 10);
    }

    #[test]
    fn expand_neighborhood_returns_seed_ids_unchanged() {
        let gate = PopulateGate::default();
        // Adapter is unused in the v1 stub; we pass a concrete instance
        // only because the signature requires one. The HF adapter is
        // safe to construct without network access.
        let adapter = crate::huggingface::HuggingFaceAdapter::new();
        let seeds = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let out = expand_neighborhood(&adapter, &gate, seeds.clone());
        assert_eq!(out, seeds);
    }
}
