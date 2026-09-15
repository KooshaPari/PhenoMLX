//! SkillRegistry behavior tests.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use hwledger_search_core::{CoreError, FusedResult, SearchContext, SearchSkill, SkillRegistry};

/// No-op skill that records invocations and bumps scores.
#[derive(Debug)]
struct RecordingSkill {
    name: String,
    delta: f32,
    calls: Arc<AtomicUsize>,
}

impl SearchSkill for RecordingSkill {
    fn name(&self) -> &str {
        &self.name
    }
    fn rerank(
        &self,
        results: &mut [FusedResult],
        _ctx: &SearchContext,
    ) -> Result<(), CoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        for r in results.iter_mut() {
            r.score += self.delta;
        }
        Ok(())
    }
}

#[test]
fn empty_registry_has_len_zero() {
    let reg = SkillRegistry::new();
    assert_eq!(reg.len(), 0);
    assert!(reg.is_empty());
}

#[test]
fn run_all_invokes_each_skill_in_order() {
    let calls_a = Arc::new(AtomicUsize::new(0));
    let calls_b = Arc::new(AtomicUsize::new(0));
    let calls_c = Arc::new(AtomicUsize::new(0));

    let mut reg = SkillRegistry::new();
    reg.register(Box::new(RecordingSkill {
        name: "a".into(),
        delta: 1.0,
        calls: calls_a.clone(),
    }));
    reg.register(Box::new(RecordingSkill {
        name: "b".into(),
        delta: 2.0,
        calls: calls_b.clone(),
    }));
    reg.register(Box::new(RecordingSkill {
        name: "c".into(),
        delta: 3.0,
        calls: calls_c.clone(),
    }));
    assert_eq!(reg.len(), 3);
    assert!(!reg.is_empty());

    let mut results = vec![
        FusedResult::new("hf::x", 0.0),
        FusedResult::new("hf::y", 0.0),
    ];
    let names_before: Vec<String> = reg.skills().iter().map(|s| s.name().to_owned()).collect();
    assert_eq!(names_before, vec!["a", "b", "c"]);

    reg.run_all(&mut results, &SearchContext::default())
        .expect("all skills must succeed");

    // Each skill fired exactly once, total delta = 1 + 2 + 3 = 6 per row.
    assert_eq!(calls_a.load(Ordering::SeqCst), 1);
    assert_eq!(calls_b.load(Ordering::SeqCst), 1);
    assert_eq!(calls_c.load(Ordering::SeqCst), 1);
    for r in &results {
        assert!((r.score - 6.0).abs() < 1e-6);
    }
}
