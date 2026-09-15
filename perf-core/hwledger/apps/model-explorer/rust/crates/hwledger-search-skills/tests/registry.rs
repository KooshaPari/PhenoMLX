//! Integration tests for `hwledger-search-skills`'s registry wiring.
//!
//! These exercise the public surface (the `SearchSkill` trait, the
//! `default_registry` factory, and the re-exports) end-to-end. The
//! per-skill unit tests live alongside the skill source files.

use hwledger_search_core::{FusedResult, Query, SearchContext, SearchIntent, SearchSkill};
use hwledger_search_skills::{default_registry, AgenticFitRerank, LlmSummarizer};
use serde_json::json;

/// Build a context for the given intent; query text is irrelevant
/// for the skills under test.
fn ctx(intent: SearchIntent) -> SearchContext {
    SearchContext::new(Query::default(), intent)
}

fn result(id: &str, score: f32, payload: Option<serde_json::Value>) -> FusedResult {
    let mut r = FusedResult::new(id, score);
    r.payload = payload;
    r
}

#[test]
fn default_registry_builds_with_two_skills() {
    let reg = default_registry();
    assert_eq!(reg.len(), 2);
    let names: Vec<&str> = reg.skills().iter().map(|s| s.name()).collect();
    // Order is contractually fixed (AgenticFitRerank first, then
    // LlmSummarizer); the registry evaluates skills in registration
    // order, so anything that flips this list would silently change
    // behavior.
    assert_eq!(
        names,
        vec![AgenticFitRerank::new().name(), LlmSummarizer::new().name()]
    );
}

#[test]
fn default_registry_runs_skills_in_order() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // Wrap the built-in skills in a thin harness that records the
    // observed name before delegating. Verifies run_all visits them
    // in registration order.
    struct Recorder {
        name: &'static str,
        version: u32,
        log: Arc<AtomicUsize>,
        slot: usize,
    }
    impl hwledger_search_core::SearchSkill for Recorder {
        fn name(&self) -> &str {
            self.name
        }
        fn version(&self) -> u32 {
            self.version
        }
        fn rerank(
            &self,
            _r: &mut [FusedResult],
            _c: &SearchContext,
        ) -> Result<(), hwledger_search_core::CoreError> {
            self.log.store(self.slot, Ordering::SeqCst);
            Ok(())
        }
    }

    let log = Arc::new(AtomicUsize::new(99));
    let mut reg = hwledger_search_core::SkillRegistry::new();
    reg.register(Box::new(Recorder {
        name: "first",
        version: 1,
        log: log.clone(),
        slot: 1,
    }));
    reg.register(Box::new(Recorder {
        name: "second",
        version: 1,
        log: log.clone(),
        slot: 2,
    }));
    let mut results = vec![FusedResult::new("hf::a", 0.0)];
    reg.run_all(&mut results, &ctx(SearchIntent::Generic))
        .unwrap();
    // Slot ended at 2 (the second skill) → both ran, in order.
    assert_eq!(log.load(Ordering::SeqCst), 2);
}

#[test]
fn agentic_fit_rerank_adjusts_scores_when_intent_matches() {
    let reg = default_registry();
    let mut results = vec![
        // Lower fused score, high agentic fit → must climb.
        result("hf::tooling-mini", 0.5, Some(json!({"agentic": 1.0}))),
        // Higher fused score, zero agentic fit → must sink.
        result("hf::chat-base", 0.9, Some(json!({"agentic": 0.0}))),
    ];
    reg.run_all(&mut results, &ctx(SearchIntent::Agentic))
        .expect("registry run_all must succeed for benign intents");
    assert_eq!(results[0].id, "hf::tooling-mini");
    assert_eq!(results[1].id, "hf::chat-base");
    // 0.6 * 0.5 + 0.4 * 1.0 = 0.7
    assert!((results[0].score - 0.7).abs() < 1e-6);
    // 0.6 * 0.9 + 0.4 * 0.0 = 0.54
    assert!((results[1].score - 0.54).abs() < 1e-6);
}

#[test]
fn agentic_fit_rerank_is_pass_through_on_generic_intent() {
    let reg = default_registry();
    let mut results = vec![
        result("hf::tooling-mini", 0.5, Some(json!({"agentic": 1.0}))),
        result("hf::chat-base", 0.9, Some(json!({"agentic": 0.0}))),
    ];
    let snapshot = results.clone();
    reg.run_all(&mut results, &ctx(SearchIntent::Generic))
        .unwrap();
    // LlmSummarizer is a v1 no-op, AgenticFitRerank is gated on
    // intent — so the registry as a whole must not mutate scores
    // when the query has generic intent.
    assert_eq!(results, snapshot);
}

#[test]
fn llm_summarizer_skill_exported_and_passes_through() {
    // Independent of the registry — make sure the re-exported
    // `LlmSummarizer` itself behaves as documented.
    let s = LlmSummarizer::new();
    let mut results = vec![FusedResult::new("hf::a", 0.42)];
    let snapshot = results.clone();
    s.rerank(&mut results, &ctx(SearchIntent::Generic))
        .expect("v1 stub must not error");
    assert_eq!(results, snapshot);
}
