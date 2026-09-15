//! [`WeightedSkill`] — adapter that scales an inner skill's rerank
//! delta by a user-supplied `f32` weight.
//!
//! The adapter is the runtime counterpart of the `weight` field in a
//! [`SkillConfigEntry`](crate::SkillConfigEntry): it lets operators
//! tune the impact of a built-in skill from a config file without
//! recompiling. The semantics — "scale the delta, not the absolute
//! score" — matter because some skills are pass-through for certain
//! intents (e.g. [`AgenticFitRerank`](crate::AgenticFitRerank) is a
//! no-op on `Generic`); scaling the absolute score in those cases
//! would silently inject non-zero weights for a no-op, distorting the
//! ranking.

use hwledger_search_core::{CoreError, FusedResult, SearchContext, SearchSkill};

/// Wraps a [`SearchSkill`] and scales the *change* it makes to each
/// result's score by `weight`.
///
/// # Semantics
///
/// For every result `r` the inner skill produces a new score
/// `inner_new = inner.rerank(r)`. The wrapper observes the original
/// `r.score`, runs the inner skill, then writes
/// `r.score = original + weight * (inner_new - original)`. At
/// `weight = 1.0` the wrapper is a transparent pass-through; at
/// `weight = 0.0` the wrapper is a no-op; at `weight = 2.0` the
/// delta is doubled. `weight` must be non-negative; the constructor
/// rejects negative or non-finite values.
///
/// The wrapper does **not** observe any sort the inner skill might
/// have applied — for an inner that reorders results (e.g.
/// `AgenticFitRerank` on `Agentic` intent), the wrapper preserves
/// the inner's ordering. This is intentional: the wrapper is a
/// thin tuner, not a reimplementation.
pub struct WeightedSkill {
    /// Operator-supplied label surfaced via [`SearchSkill::name`].
    pub name: String,
    /// Inner skill, kept as `Box<dyn SearchSkill>` so the wrapper
    /// works for any built-in or future custom skill.
    inner: Box<dyn SearchSkill>,
    /// Multiplier applied to the score delta produced by `inner`.
    pub weight: f32,
}

impl std::fmt::Debug for WeightedSkill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // JSON-style debug output so the wrapper is human-readable when
        // dumped from a config-file-loaded registry. Operators inspecting
        // `format!("{reg:?}")` in logs see the same shape they wrote in
        // skills.toml.
        write!(
            f,
            r#"{{"name": "{}", "inner_name": "{}", "weight": {}}}"#,
            self.name,
            self.inner.name(),
            self.weight,
        )
    }
}

impl WeightedSkill {
    /// Construct a new wrapper. Returns `Err` if `weight` is negative
    /// or non-finite (NaN / infinity); both would make the rerank
    /// delta non-deterministic and are caught at config-load time
    /// rather than at query time.
    pub fn new(
        name: impl Into<String>,
        inner: Box<dyn SearchSkill>,
        weight: f32,
    ) -> Result<Self, &'static str> {
        if !weight.is_finite() {
            return Err("weight must be finite (no NaN, no infinity)");
        }
        if weight < 0.0 {
            return Err("weight must be non-negative");
        }
        Ok(Self {
            name: name.into(),
            inner,
            weight,
        })
    }
}

impl SearchSkill for WeightedSkill {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> u32 {
        // Bump the version with the wrapper so observers can
        // distinguish weighted from unweighted runs without having to
        // inspect the debug output.
        self.inner.version()
    }

    fn rerank(
        &self,
        results: &mut [FusedResult],
        ctx: &SearchContext,
    ) -> Result<(), CoreError> {
        if self.weight == 1.0 {
            // Fast path: avoid cloning every score when the wrapper
            // is configured at its neutral value.
            return self.inner.rerank(results, ctx);
        }

        // Snapshot original scores so we can compute the delta the
        // inner produced, then re-apply it scaled by `weight`.
        let original: Vec<f32> = results.iter().map(|r| r.score).collect();
        self.inner.rerank(results, ctx)?;
        for (r, &orig) in results.iter_mut().zip(original.iter()) {
            let delta = r.score - orig;
            r.score = orig + self.weight * delta;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hwledger_search_core::{Query, SearchIntent};
    use serde_json::json;

    fn ctx() -> SearchContext {
        SearchContext::new(Query::default(), SearchIntent::Generic)
    }

    /// Test skill that adds `delta` to every score. Lets us observe
    /// the wrapper's scaling behaviour without depending on the
    /// built-in skills' intent-gating.
    struct Bump {
        delta: f32,
    }
    impl SearchSkill for Bump {
        fn name(&self) -> &str {
            "bump"
        }
        fn rerank(
            &self,
            results: &mut [FusedResult],
            _ctx: &SearchContext,
        ) -> Result<(), CoreError> {
            for r in results.iter_mut() {
                r.score += self.delta;
            }
            Ok(())
        }
    }

    #[test]
    fn rejects_negative_weight() {
        let inner = Box::new(Bump { delta: 0.1 });
        let err = WeightedSkill::new("w", inner, -0.5).unwrap_err();
        assert!(err.contains("non-negative"), "got: {err}");
    }

    #[test]
    fn rejects_nan_weight() {
        let inner = Box::new(Bump { delta: 0.1 });
        let err = WeightedSkill::new("w", inner, f32::NAN).unwrap_err();
        assert!(err.contains("finite"), "got: {err}");
    }

    #[test]
    fn rejects_infinite_weight() {
        let inner = Box::new(Bump { delta: 0.1 });
        let err = WeightedSkill::new("w", inner, f32::INFINITY).unwrap_err();
        assert!(err.contains("finite"), "got: {err}");
    }

    #[test]
    fn weight_zero_is_no_op() {
        let w = WeightedSkill::new("w", Box::new(Bump { delta: 0.4 }), 0.0).unwrap();
        let mut results = vec![FusedResult::new("a", 0.1), FusedResult::new("b", 0.7)];
        let snapshot = results.clone();
        w.rerank(&mut results, &ctx()).unwrap();
        assert_eq!(results, snapshot, "weight=0 must short-circuit to identity");
    }

    #[test]
    fn weight_one_is_passthrough() {
        let w = WeightedSkill::new("w", Box::new(Bump { delta: 0.4 }), 1.0).unwrap();
        let mut results = vec![FusedResult::new("a", 0.1), FusedResult::new("b", 0.7)];
        w.rerank(&mut results, &ctx()).unwrap();
        assert!((results[0].score - 0.5).abs() < 1e-6);
        assert!((results[1].score - 1.1).abs() < 1e-6);
    }

    #[test]
    fn weight_two_doubles_delta() {
        let w = WeightedSkill::new("w", Box::new(Bump { delta: 0.4 }), 2.0).unwrap();
        let mut results = vec![FusedResult::new("a", 0.1), FusedResult::new("b", 0.7)];
        w.rerank(&mut results, &ctx()).unwrap();
        // inner would have produced 0.5 / 1.1; the wrapper doubles the
        // delta: 0.1 + 2*(0.5-0.1) = 0.9, 0.7 + 2*(1.1-0.7) = 1.5.
        assert!((results[0].score - 0.9).abs() < 1e-6);
        assert!((results[1].score - 1.5).abs() < 1e-6);
    }

    #[test]
    fn weight_half_halves_delta() {
        let w = WeightedSkill::new("w", Box::new(Bump { delta: 0.4 }), 0.5).unwrap();
        let mut results = vec![FusedResult::new("a", 0.1)];
        w.rerank(&mut results, &ctx()).unwrap();
        // 0.1 + 0.5 * (0.5 - 0.1) = 0.3
        assert!((results[0].score - 0.3).abs() < 1e-6);
    }

    #[test]
    fn exposes_operator_name() {
        let w = WeightedSkill::new("my-rename", Box::new(Bump { delta: 0.0 }), 1.0).unwrap();
        assert_eq!(w.name(), "my-rename");
        assert_eq!(w.version(), 1);
    }

    #[test]
    fn debug_includes_inner_name_and_weight() {
        let w = WeightedSkill::new("w", Box::new(Bump { delta: 0.0 }), 1.25).unwrap();
        let s = format!("{w:?}");
        assert!(s.contains("\"name\": \"w\""));
        assert!(s.contains("\"inner_name\": \"bump\""));
        assert!(s.contains("1.25"));
    }

    #[test]
    fn integrated_with_agentic_fit_rerank() {
        // Smoke test: wrap the real AgenticFitRerank and confirm a
        // weight of 0.5 produces a half-strength rerank on a result
        // whose `agentic` fit would normally lift it.
        let w = WeightedSkill::new(
            "agentic-fit-half",
            Box::new(crate::AgenticFitRerank::new()),
            0.5,
        )
        .unwrap();
        let mut results = vec![FusedResult::new("hf::x", 0.5).with_payload(json!({"agentic": 1.0}))];
        w.rerank(&mut results, &SearchContext::new(Query::default(), SearchIntent::Agentic))
            .unwrap();
        // Inner would have produced 0.6*0.5 + 0.4*1.0 = 0.7 (delta 0.2).
        // With weight 0.5: 0.5 + 0.5*0.2 = 0.6.
        assert!((results[0].score - 0.6).abs() < 1e-6);
    }
}
