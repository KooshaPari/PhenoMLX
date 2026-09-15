//! [`LlmSummarizer`] — v1 no-op placeholder skill.
//!
//! The skill exists so [`default_registry`](crate::default_registry) has
//! a stable second slot, and so downstream callers can register
//! behavior hooks at a predictable point in the rerank pipeline. A
//! future version will call out to an LLM and inject a summary into
//! each result's payload.

use hwledger_search_core::{CoreError, FusedResult, SearchContext, SearchSkill};

/// Stable identifier used by the registry.
pub const NAME: &str = "summary:llm";

/// Skill version.
pub const VERSION: u32 = 1;

/// v1 no-op stub. Will eventually call out to an LLM and stash a
/// summary string into each result's payload.
#[derive(Debug, Default, Clone, Copy)]
pub struct LlmSummarizer;

impl LlmSummarizer {
    /// Construct a new instance. Mirrors the `Box::new(...)` pattern
    /// used elsewhere in the registry.
    pub fn new() -> Self {
        Self
    }
}

impl SearchSkill for LlmSummarizer {
    fn name(&self) -> &str {
        NAME
    }

    fn version(&self) -> u32 {
        VERSION
    }

    fn rerank(&self, _results: &mut [FusedResult], _ctx: &SearchContext) -> Result<(), CoreError> {
        // v1 is intentionally a no-op. Returning Ok keeps the
        // registry pipeline moving; once we wire up the LLM client
        // we'll mutate `_results` here.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hwledger_search_core::{Query, SearchIntent};

    #[test]
    fn no_op_pass_through() {
        let mut results = vec![
            FusedResult::new("hf::a", 0.4),
            FusedResult::new("hf::b", 0.9),
        ];
        let snapshot = results.clone();
        let ctx = SearchContext::new(Query::default(), SearchIntent::Generic);
        LlmSummarizer
            .rerank(&mut results, &ctx)
            .expect("v1 no-op must never error");
        assert_eq!(
            results, snapshot,
            "v1 LlmSummarizer must not mutate results"
        );
    }

    #[test]
    fn stable_metadata() {
        let s = LlmSummarizer::new();
        assert_eq!(s.name(), NAME);
        assert_eq!(s.version(), VERSION);
    }
}
