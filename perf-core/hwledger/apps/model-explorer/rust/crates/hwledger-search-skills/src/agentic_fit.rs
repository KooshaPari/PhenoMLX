//! [`AgenticFitRerank`] — score-modulating rerank skill.
//!
//! When the incoming [`SearchContext`] advertises an agent or coding
//! intent, every result's score is replaced by
//!
//! ```text
//! new_score = 0.6 * fused_score + 0.4 * intent_fit
//! ```
//!
//! where `intent_fit` is sourced from the result's payload (the
//! `agentic` or `coding` float the upstream `usecase_fit_tagger`
//! stamps in). After re-scoring, the slice is re-sorted descending by
//! the new score so the UI / downstream consumers see the best-fit
//! rows first.
//!
//! If the context's intent is `Generic`, `Reasoning`, or `Embedding`
//! the skill is a pass-through — there is no "intent fit" signal we
//! can defensibly boost on without miscalibrating results.

use hwledger_search_core::{CoreError, FusedResult, SearchContext, SearchIntent, SearchSkill};

/// Stable identifier used by the registry for observability + cache
/// invalidation.
pub const NAME: &str = "rerank:agentic-fit";

/// Skill version. Bumped only when the score-mix policy changes in a
/// user-visible way.
pub const VERSION: u32 = 1;

/// Weight applied to the original fused score.
const SCORE_WEIGHT: f32 = 0.6;

/// Weight applied to the per-result intent-fit term.
const FIT_WEIGHT: f32 = 0.4;

/// Re-scores results by `0.6 * score + 0.4 * intent_fit` when the
/// query has agent or coding intent.
///
/// See the [module-level docs](self) for the full policy.
#[derive(Debug, Default, Clone, Copy)]
pub struct AgenticFitRerank;

impl AgenticFitRerank {
    /// Construct a new instance. The struct is zero-sized, but the
    /// constructor mirrors the `Box::new(...)` pattern used elsewhere
    /// in the registry so callers can swap in mock skills without
    /// changing call sites.
    pub fn new() -> Self {
        Self
    }
}

impl SearchSkill for AgenticFitRerank {
    fn name(&self) -> &str {
        NAME
    }

    fn version(&self) -> u32 {
        VERSION
    }

    fn rerank(&self, results: &mut [FusedResult], ctx: &SearchContext) -> Result<(), CoreError> {
        // Only the agent & coding intents carry a defensible
        // intent-fit signal. Anything else ⇒ pass-through.
        let intent_key: &str = match ctx.intent {
            SearchIntent::Agentic => "agentic",
            SearchIntent::Coding => "coding",
            SearchIntent::Generic | SearchIntent::Reasoning | SearchIntent::Embedding => {
                return Ok(());
            }
        };

        for r in results.iter_mut() {
            let fit = read_intent_fit(&r.payload, intent_key);
            r.score = SCORE_WEIGHT * r.score + FIT_WEIGHT * fit;
        }

        // Stable sort so ties keep their pre-rerank relative order
        // (cheaper to reason about in tests and cheaper on cache
        // locality for the downstream renderer).
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(())
    }
}

/// Pull a `f32` from `payload[key]`, accepting both a bare number and
/// a string-encoded number (the latter happens when upstream tags
/// travel through JSON round-trips). Anything else — `null`,
/// mismatched types, missing payload — yields `0.0`.
fn read_intent_fit(payload: &Option<serde_json::Value>, key: &str) -> f32 {
    let Some(value) = payload.as_ref().and_then(|v| v.get(key)) else {
        return 0.0;
    };
    match value {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) as f32,
        serde_json::Value::String(s) => s.parse::<f32>().unwrap_or(0.0),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hwledger_search_core::Query;
    use serde_json::json;

    fn ctx(intent: SearchIntent) -> SearchContext {
        SearchContext::new(Query::default(), intent)
    }

    fn result_with(id: &str, score: f32, payload: Option<serde_json::Value>) -> FusedResult {
        let mut r = FusedResult::new(id, score);
        r.payload = payload;
        r
    }

    #[test]
    fn pass_through_on_generic_intent() {
        let mut results = vec![
            result_with("hf::a", 0.5, Some(json!({"agentic": 1.0}))),
            result_with("hf::b", 0.9, Some(json!({"agentic": 0.0}))),
        ];
        let original = results.clone();
        AgenticFitRerank
            .rerank(&mut results, &ctx(SearchIntent::Generic))
            .unwrap();
        assert_eq!(results, original, "Generic intent must not mutate scores");
    }

    #[test]
    fn pass_through_on_reasoning_intent() {
        let mut results = vec![
            result_with("hf::a", 0.5, Some(json!({"agentic": 1.0}))),
            result_with("hf::b", 0.9, Some(json!({"agentic": 0.0}))),
        ];
        let original = results.clone();
        AgenticFitRerank
            .rerank(&mut results, &ctx(SearchIntent::Reasoning))
            .unwrap();
        assert_eq!(results, original);
    }

    #[test]
    fn agentic_intent_re_scores_and_reorders() {
        let mut results = vec![
            // Lower fused score but high agentic fit ⇒ must climb up.
            result_with("hf::tooling-mini", 0.5, Some(json!({"agentic": 1.0}))),
            // Higher fused score but zero agentic fit ⇒ must sink.
            result_with("hf::chat-base", 0.9, Some(json!({"agentic": 0.0}))),
        ];
        AgenticFitRerank
            .rerank(&mut results, &ctx(SearchIntent::Agentic))
            .unwrap();
        assert_eq!(results[0].id, "hf::tooling-mini");
        assert_eq!(results[1].id, "hf::chat-base");
        // 0.6 * 0.5 + 0.4 * 1.0 = 0.7
        assert!((results[0].score - 0.7).abs() < 1e-6);
        // 0.6 * 0.9 + 0.4 * 0.0 = 0.54
        assert!((results[1].score - 0.54).abs() < 1e-6);
    }

    #[test]
    fn coding_intent_reads_coding_key() {
        let mut results = vec![
            result_with("hf::code-7b", 0.6, Some(json!({"coding": 0.9}))),
            result_with("hf::chat-7b", 0.6, Some(json!({"coding": 0.1}))),
        ];
        AgenticFitRerank
            .rerank(&mut results, &ctx(SearchIntent::Coding))
            .unwrap();
        assert_eq!(results[0].id, "hf::code-7b");
    }

    #[test]
    fn missing_payload_treated_as_zero_fit() {
        let mut results = vec![
            result_with("hf::a", 0.8, None),
            result_with("hf::b", 0.7, Some(json!({"unrelated": true}))),
        ];
        // With no fit info, scores stay at 0.6 * original and the
        // higher pre-rerank row stays on top.
        AgenticFitRerank
            .rerank(&mut results, &ctx(SearchIntent::Agentic))
            .unwrap();
        assert_eq!(results[0].id, "hf::a");
        assert!((results[0].score - 0.48).abs() < 1e-6);
    }

    #[test]
    fn string_encoded_fit_is_parsed() {
        let mut results = vec![result_with("hf::x", 0.5, Some(json!({"agentic": "0.5"})))];
        AgenticFitRerank
            .rerank(&mut results, &ctx(SearchIntent::Agentic))
            .unwrap();
        // 0.6 * 0.5 + 0.4 * 0.5 = 0.5
        assert!((results[0].score - 0.5).abs() < 1e-6);
    }
}
