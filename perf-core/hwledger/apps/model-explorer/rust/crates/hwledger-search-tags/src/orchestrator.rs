//! Top-level orchestrator wiring every heuristic tagger together.
//!
//! `tag_all` is the single entry-point downstream crates (ingest, evals,
//! the index) call. It runs the taggers in dependency order so that
//! downstream consumers (e.g. the use-case fit scorer) can rely on the
//! upstream tags already being populated.

use crate::arch_tagger::{self, ArchTags};
use crate::license_tagger::{self, LicenseTags};
use crate::modelkind_tagger::{self, ModelKindTags};
use crate::moe_tagger::{self, MoeTags};
use crate::param_tagger::{self, ParamTags};
use crate::provenance_tagger::{self, ProvenanceTags};
use crate::quant_tagger::{self, QuantTags};
use crate::reap_tagger::{self, ReapTags};
use crate::tager_context::TaggerContext;
use crate::usecase_fit_tagger::{self, FitScore};

/// Full tag bundle for a single model.
///
/// `Default` produces the "every tagger said 'I don't know'" shape, which
/// is also what `tag_all` returns for an empty `TaggerContext`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AllTags {
    /// Architecture family results.
    pub arch: ArchTags,
    /// MoE results.
    pub moe: MoeTags,
    /// Quantization results.
    pub quant: QuantTags,
    /// Parameter-bucket results.
    pub param: ParamTags,
    /// License classification.
    pub license: LicenseTags,
    /// Model kind classification.
    pub kind: ModelKindTags,
    /// ReAp-reasoning tag.
    pub reap: ReapTags,
    /// Provenance classification.
    pub provenance: ProvenanceTags,
    /// Use-case fit score.
    pub fit: FitScore,
}

/// Run every tagger and return the consolidated [`AllTags`] bundle.
///
/// Taggers run in dependency order so that the use-case fit scorer sees a
/// fully-populated upstream tag set:
/// 1. `arch`     — pure config inspection
/// 2. `moe`      — pure config inspection
/// 3. `quant`    — pure tree-entries scan
/// 4. `param`    — depends on `moe` so active-params can be propagated
/// 5. `license`  — pure config / card scan
/// 6. `kind`     — id / pipeline / card keyword scan
/// 7. `reap`     — id / card keyword scan
/// 8. `provenance` — id / card keyword scan
/// 9. `fit`      — composite of `kind`, `arch`, `moe`, `quant`
pub fn tag_all(ctx: &TaggerContext) -> AllTags {
    let arch = arch_tagger::tag(ctx);
    let moe = moe_tagger::tag(ctx);
    let quant = quant_tagger::tag(ctx);
    let param = param_tagger::tag(ctx, &moe);
    let license = license_tagger::tag(ctx);
    let kind = modelkind_tagger::tag(ctx);
    let reap = reap_tagger::tag(ctx);
    let provenance = provenance_tagger::tag(ctx);
    let fit = usecase_fit_tagger::tag(ctx, &kind, &arch, &moe, &quant);

    AllTags {
        arch,
        moe,
        quant,
        param,
        license,
        kind,
        reap,
        provenance,
        fit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_context_is_default() {
        let ctx = TaggerContext::default();
        let tags = tag_all(&ctx);
        // Every structural tag axis is the documented default.
        assert_eq!(tags.arch, ArchTags::default());
        assert_eq!(tags.moe, MoeTags::default());
        assert_eq!(tags.quant, QuantTags::default());
        assert_eq!(tags.param, ParamTags::default());
        assert_eq!(tags.license, LicenseTags::default());
        assert_eq!(tags.kind, ModelKindTags::default());
        assert_eq!(tags.reap, ReapTags::default());
        assert_eq!(tags.provenance.provenance, "unknown");
        assert_eq!(tags.provenance.base_model, None);
        // The fit score is the only axis the formula doesn't tie to zero
        // for an empty input (it carries a small "non-MoE dense" baseline
        // for the coding axis). We assert both components are well below
        // 0.5 so the formula in the public docstring stays the source of
        // truth.
        assert!(tags.fit.agentic < 0.5);
        assert!(tags.fit.coding < 0.5);
    }
}
