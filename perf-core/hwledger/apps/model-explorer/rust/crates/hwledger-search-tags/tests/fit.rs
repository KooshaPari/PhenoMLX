//! Integration tests for `usecase_fit_tagger`.

use hwledger_search_core::{ArchKind, AttentionKind, MlpKind, ModelKind, RopeVariant};

use hwledger_search_tags::arch_tagger::ArchTags;
use hwledger_search_tags::modelkind_tagger::ModelKindTags;
use hwledger_search_tags::moe_tagger::MoeTags;
use hwledger_search_tags::quant_tagger::QuantTags;
use hwledger_search_tags::tager_context::TaggerContext;
use hwledger_search_tags::usecase_fit_tagger::tag;

fn arch(attention: AttentionKind, rope: RopeVariant) -> ArchTags {
    ArchTags {
        arch_kind: ArchKind::Dense,
        attention,
        mlp: MlpKind::SwiGlu,
        rope,
    }
}

#[test]
fn agentic_gqa_moe_safetensors() {
    let kind = ModelKindTags {
        primary_kind: ModelKind::Agentic,
    };
    let arch = arch(AttentionKind::Gqa, RopeVariant::Standard);
    let moe = MoeTags {
        is_moe: true,
        ..MoeTags::default()
    };
    let quant = QuantTags {
        safetensors_present: true,
        ..QuantTags::default()
    };
    let ctx = TaggerContext::default();
    let fit = tag(&ctx, &kind, &arch, &moe, &quant);
    assert!(
        fit.agentic >= 0.8,
        "agentic should be >=0.8 for GQA+MoE+safetensors+Agentic, got {}",
        fit.agentic
    );
    assert!(
        fit.coding > 0.0 && fit.coding < 0.6,
        "coding should be moderate, got {}",
        fit.coding
    );
}

#[test]
fn coding_mha_dense_above_threshold() {
    let kind = ModelKindTags {
        primary_kind: ModelKind::Coding,
    };
    let arch = arch(AttentionKind::Mha, RopeVariant::None);
    let moe = MoeTags::default();
    let quant = QuantTags::default();
    let ctx = TaggerContext::default();
    let fit = tag(&ctx, &kind, &arch, &moe, &quant);
    assert!(
        fit.coding >= 0.6,
        "coding should be >=0.6 for Coding + Mha + no MoE, got {}",
        fit.coding
    );
    assert!(
        fit.agentic < 0.5,
        "agentic should be low for non-Agentic, got {}",
        fit.agentic
    );
}
