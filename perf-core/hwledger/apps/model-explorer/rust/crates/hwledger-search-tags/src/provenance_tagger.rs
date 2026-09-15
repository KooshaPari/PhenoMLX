//! Provenance tagger.
//!
//! Classifies a model's provenance — where it came from — into one of
//! `"original"`, `"finetune"`, `"merge"`, or `"unknown"`.
//!
//! The dispatch is deliberately simple:
//!
//! 1. **Original**: id starts with one of the canonical first-party orgs
//!    (`meta-llama/`, `mistralai/`, `google/`, `Qwen/`, `microsoft/`,
//!    `openai/`) and the model card / id has no merge / lora markers.
//! 2. **Finetune**: card text contains a `base_model:` field or
//!    "fine-tuned from" → use the parsed base model.
//! 3. **Merge**: card text mentions `mergekit` or "merge of" → use the
//!    parsed base model.
//! 4. **Unknown**: nothing else matched.

use crate::tager_context::TaggerContext;

/// Provenance facet for a single model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProvenanceTags {
    /// Provenance label: one of `"original"`, `"finetune"`, `"merge"`,
    /// `"unknown"`. Defaults to `"unknown"`.
    pub provenance: String,
    /// Underlying base model id, if we could parse one.
    pub base_model: Option<String>,
}

/// Canonical first-party organisations whose repos are treated as
/// "original" provenance.
const CANONICAL_ORGS: &[&str] = &[
    "meta-llama/",
    "mistralai/",
    "google/",
    "qwen/",
    "microsoft/",
    "openai/",
];

/// Heuristic provenance detector.
pub fn tag(ctx: &TaggerContext) -> ProvenanceTags {
    let card = ctx.effective_card_text().unwrap_or("").to_ascii_lowercase();
    let id_lower = ctx.id.to_ascii_lowercase();

    // 1. Finetune detection (check before "original" because a finetune
    //    could still be hosted under a canonical org).
    if card.contains("base_model:") || card.contains("fine-tuned from") {
        let base = parse_base_model_field(ctx).unwrap_or_else(|| ctx.id.clone());
        return ProvenanceTags {
            provenance: "finetune".to_string(),
            base_model: Some(base),
        };
    }

    // 2. Merge detection.
    if card.contains("mergekit") || card.contains("merge of") {
        let base = parse_base_model_field(ctx).unwrap_or_else(|| ctx.id.clone());
        return ProvenanceTags {
            provenance: "merge".to_string(),
            base_model: Some(base),
        };
    }

    // 3. Original detection — id is under a canonical org AND no merge / lora
    //    markers anywhere.
    let lower_full = format!("{} {}", id_lower, card);
    let has_merge_marker = lower_full.contains("merge")
        || lower_full.contains("lora")
        || lower_full.contains("adapter");
    if !has_merge_marker && CANONICAL_ORGS.iter().any(|p| id_lower.starts_with(p)) {
        return ProvenanceTags {
            provenance: "original".to_string(),
            base_model: Some(ctx.id.clone()),
        };
    }

    // 4. Fallthrough.
    ProvenanceTags {
        provenance: "unknown".to_string(),
        base_model: None,
    }
}

/// Try to extract a `base_model:` field from the model card. We accept
/// either YAML-style `base_model: org/name` or the comma-separated stream
/// of `base_model: org/name1, org/name2` (HF sometimes emits both).
fn parse_base_model_field(ctx: &TaggerContext) -> Option<String> {
    let card = ctx.effective_card_text()?;
    for line in card.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed
            .strip_prefix("base_model:")
            .or_else(|| trimmed.strip_prefix("Base model:"))
        {
            let val = rest.trim();
            if !val.is_empty() {
                // If it's a comma-separated list, take the first.
                let first = val.split(',').next().unwrap_or(val).trim();
                if !first.is_empty() {
                    return Some(first.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_org_is_original() {
        let ctx = TaggerContext::from_id("meta-llama/Llama-3.1-8B", "meta-llama");
        let p = tag(&ctx);
        assert_eq!(p.provenance, "original");
        assert_eq!(p.base_model.as_deref(), Some("meta-llama/Llama-3.1-8B"));
    }

    #[test]
    fn finetune_via_card() {
        let card = "# Model card\n\nbase_model: meta-llama/Llama-3-8B\n\nFinetuned for chat.\n";
        let ctx = TaggerContext {
            id: "community/my-finetune".to_string(),
            org: "community".to_string(),
            card_text: Some(card.to_string()),
            ..Default::default()
        };
        let p = tag(&ctx);
        assert_eq!(p.provenance, "finetune");
        assert_eq!(p.base_model.as_deref(), Some("meta-llama/Llama-3-8B"));
    }

    #[test]
    fn merge_via_card() {
        let card = "# Merge\n\nBuilt with mergekit — merge of meta-llama/Llama-3-8B and mistralai/Mistral-7B\n";
        let ctx = TaggerContext {
            id: "community/merged".to_string(),
            org: "community".to_string(),
            card_text: Some(card.to_string()),
            ..Default::default()
        };
        let p = tag(&ctx);
        assert_eq!(p.provenance, "merge");
    }

    #[test]
    fn unknown_when_no_signal() {
        let ctx = TaggerContext::from_id("random-org/some-model", "random-org");
        let p = tag(&ctx);
        assert_eq!(p.provenance, "unknown");
        assert!(p.base_model.is_none());
    }
}
