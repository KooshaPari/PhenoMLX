//! Use-case fit tagger.
//!
//! Computes a coarse `agentic` / `coding` fit score in `[0.0, 1.0]` from
//! the up-stream tag bundle. The score is **not** a benchmark — it's a
//! stain test that the indexer can use to break ties between otherwise
//! equally-relevant BM25 / semantic results.
//!
//! The exact weights are documented in the function-level docstring of
//! [`tag`] so they're easy to tune against user feedback.

use hwledger_search_core::{AttentionKind, ModelKind, RopeVariant};

use crate::arch_tagger::ArchTags;
use crate::moe_tagger::MoeTags;
use crate::quant_tagger::QuantTags;
use crate::modelkind_tagger::ModelKindTags;
use crate::tager_context::TaggerContext;

/// Fit-score pair for a single model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitScore {
    /// `agentic` fit score in `[0.0, 1.0]`.
    pub agentic: f32,
    /// `coding` fit score in `[0.0, 1.0]`.
    pub coding: f32,
}

impl Default for FitScore {
    fn default() -> Self {
        Self {
            agentic: 0.0,
            coding: 0.0,
        }
    }
}

/// Heuristic composite fit-score computation.
///
/// **Agentic** (`ctx` is ignored — the function takes the bundle of upstream
/// tags so a caller can mix-and-match without re-inferring):
/// - `+0.4` if `kind.primary_kind == Agentic`
/// - `+0.2` if `arch.attention != Mha` (GQA / MQA / MLA / SSM all signal a
///   modern backbone that's known to be amenable to tool use)
/// - `+0.2` if `moe.is_moe` (sparse experts → better long-context recall)
/// - `+0.2` if `quant.safetensors_present` (full-precision weights are
///   almost always what downstream tool-calling stacks expect)
///
///   The result is clamped to `[0.0, 1.0]`.
///
/// **Coding**:
/// - `+0.5` if `kind.primary_kind == Coding`
/// - `+0.2` if `moe.is_moe` else `+0.1` (so dense models still get a small
///   nudge)
/// - `+0.2` if `arch.rope == Standard` (the canonical RoPE variant is what
///   every code-eval harness expects)
/// - `+0.1` if `quant.gguf_present` (GGUF is the lingua franca for local
///   code-completion runtimes)
///
///   The result is clamped to `[0.0, 1.0]`.
///
/// `ctx` is currently unused — kept for future provisions (e.g. factoring
/// in id-based heuristics that don't fit any sub-tag yet).
#[allow(unused_variables, clippy::too_many_arguments)]
pub fn tag(
    ctx: &TaggerContext,
    kind: &ModelKindTags,
    arch: &ArchTags,
    moe: &MoeTags,
    quant: &QuantTags,
) -> FitScore {
    let _ = ctx;

    // --- agentic term ---
    let agentic_raw: f32 = {
        let mut s = 0.0_f32;
        if kind.primary_kind == ModelKind::Agentic {
            s += 0.4;
        }
        if arch.attention != AttentionKind::Mha {
            s += 0.2;
        }
        if moe.is_moe {
            s += 0.2;
        }
        if quant.safetensors_present {
            s += 0.2;
        }
        s
    };

    // --- coding term ---
    let coding_raw: f32 = {
        let mut s = 0.0_f32;
        if kind.primary_kind == ModelKind::Coding {
            s += 0.5;
        }
        if moe.is_moe {
            s += 0.2;
        } else {
            s += 0.1;
        }
        if arch.rope == RopeVariant::Standard {
            s += 0.2;
        }
        if quant.gguf_present {
            s += 0.1;
        }
        s
    };

    FitScore {
        agentic: clamp01(agentic_raw),
        coding: clamp01(coding_raw),
    }
}

/// Clamp a `f32` into `[0.0, 1.0]`. NaN-safe (returns 0.0).
fn clamp01(x: f32) -> f32 {
    if x.is_nan() {
        0.0
    } else if !(0.0..=1.0).contains(&x) {
        if x < 0.0 {
            0.0
        } else {
            1.0
        }
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp01_normalizes() {
        assert_eq!(clamp01(-1.0), 0.0);
        assert_eq!(clamp01(0.5), 0.5);
        assert_eq!(clamp01(1.5), 1.0);
        assert_eq!(clamp01(f32::NAN), 0.0);
    }
}
