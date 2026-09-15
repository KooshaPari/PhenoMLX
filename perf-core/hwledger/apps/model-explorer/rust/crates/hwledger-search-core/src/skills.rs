//! Pluggable rerankers.
//!
//! Skills let downstream agents and tools mutate a [`FusedResult`] slice
//! in-place (re-score, drop, inject extra context, …) before the CLI/server
//! layers render it. The trait is intentionally minimal: zero dependencies
//! beyond what `search-core` already pulls, and a synchronous signature so
//! skills can be `Send + Sync` and live in a plain `Vec`.

use crate::error::CoreError;
use crate::query::{FusedResult, Query};

/// Coarse intent classifier the frontend layer derives from the incoming
/// query. Skills can short-circuit reranking work that doesn't apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchIntent {
    /// No specific intent — generic text search.
    Generic,
    /// Coding-focused search (programming-language assistants, code
    /// rerankers, etc.).
    Coding,
    /// Tool-using / agentic search.
    Agentic,
    /// Reasoning / o1-style search.
    Reasoning,
    /// Embedding-model lookup.
    Embedding,
}

impl Default for SearchIntent {
    fn default() -> Self {
        Self::Generic
    }
}

/// Context passed to every skill's `rerank` invocation.
///
/// Contains the originating query plus the resolved intent (which skills
/// use to decide whether they apply).
#[derive(Debug, Clone)]
pub struct SearchContext {
    /// The originating query.
    pub query: Query,
    /// The resolved intent.
    pub intent: SearchIntent,
}

impl Default for SearchContext {
    fn default() -> Self {
        Self {
            query: Query::default(),
            intent: SearchIntent::default(),
        }
    }
}

impl SearchContext {
    /// Build a context for a given query and intent.
    pub fn new(query: Query, intent: SearchIntent) -> Self {
        Self { query, intent }
    }
}

/// Rerank-skills mutating a `FusedResult` slice in place.
///
/// Skills are evaluated in registration order. Each implementation must be
/// pure-ish (idempotent for repeated invocations on the same input) so they
/// remain safe to invoke multiple times across the same request.
pub trait SearchSkill: Send + Sync {
    /// Stable, lowercase identifier (e.g. `"booster:cross-encoder"`).
    fn name(&self) -> &str;

    /// Bumped whenever a skill's behavior changes in a user-visible way.
    /// `SkillRegistry` does not interpret the version — it is purely for
    /// observability and cache invalidation by callers.
    fn version(&self) -> u32 {
        1
    }

    /// Rerank the slice in place. `results` carries the pre-skill fused set,
    /// sorted descending by `score`.
    ///
    /// Skills may:
    /// - mutate any field of individual results,
    /// - reorder the slice,
    /// - drop or insert results,
    /// - leave the slice unchanged.
    ///
    /// Implementations should be lenient: dropping an item is fine, panicking
    /// is not. Return an error only for unrecoverable problems (e.g. an
    /// out-of-process model server is unavailable).
    fn rerank(&self, results: &mut [FusedResult], ctx: &SearchContext) -> Result<(), CoreError>;
}

/// Ordered collection of [`SearchSkill`]s, evaluated in registration order.
#[derive(Default)]
pub struct SkillRegistry {
    skills: Vec<Box<dyn SearchSkill>>,
}

impl SkillRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a skill to the registry.
    pub fn register(&mut self, skill: Box<dyn SearchSkill>) {
        self.skills.push(skill);
    }

    /// Number of registered skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// `true` if no skills are registered.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Borrow the registered skills by name (read-only).
    pub fn skills(&self) -> &[Box<dyn SearchSkill>] {
        &self.skills
    }

    /// Run every registered skill, in order. The first failing skill
    /// short-circuits and its error is returned.
    pub fn run_all(
        &self,
        results: &mut [FusedResult],
        ctx: &SearchContext,
    ) -> Result<(), CoreError> {
        for s in &self.skills {
            s.rerank(results, ctx)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Test skill that bumps every score by `delta` and records its name.
    struct BumpSkill {
        name: String,
        delta: f32,
        calls: Arc<AtomicUsize>,
    }

    impl SearchSkill for BumpSkill {
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

    /// Test skill that errors when called.
    struct FailSkill;
    impl SearchSkill for FailSkill {
        fn name(&self) -> &str {
            "fail"
        }
        fn rerank(
            &self,
            _results: &mut [FusedResult],
            _ctx: &SearchContext,
        ) -> Result<(), CoreError> {
            Err(CoreError::backend("boom"))
        }
    }

    #[test]
    fn default_intent_is_generic() {
        assert_eq!(SearchIntent::default(), SearchIntent::Generic);
    }

    #[test]
    fn default_context() {
        let c = SearchContext::default();
        assert_eq!(c.intent, SearchIntent::Generic);
        assert_eq!(c.query, Query::default());
    }

    #[test]
    fn run_all_short_circuits_on_error() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut reg = SkillRegistry::new();
        reg.register(Box::new(BumpSkill {
            name: "first".into(),
            delta: 1.0,
            calls: calls.clone(),
        }));
        reg.register(Box::new(FailSkill));

        let mut results = vec![FusedResult::new("hf::a", 0.0)];
        let err = reg
            .run_all(&mut results, &SearchContext::default())
            .expect_err("must error");
        assert!(matches!(err, CoreError::Backend(_)));
        // First skill ran once before failing.
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
