//! ReAp (Reasoning-as-Planning) tagger.
//!
//! Indicates whether a model was explicitly trained with the ReAp
//! paradigm (`reasoning-as-planning` / `reasoning_path`) or one of the
//! better-known open-source reasoning-equivalent variants (DeepSeek R1,
//! Qwen QwQ, generic "thinking" markers).
//!
//! The "method" string is a free-form tag the orchestrator can surface
//! into facets / metadata for downstream filtering.

use crate::tager_context::TaggerContext;

/// ReAp-specific tags for a single model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReapTags {
    /// `true` if the model is a confirmed reasoning model.
    pub is_reasoning_model: bool,
    /// Free-form method token, e.g. `"reasoning"`. `None` unless
    /// `is_reasoning_model` is true.
    pub method: Option<String>,
}

/// Heuristic ReAp detection.
pub fn tag(ctx: &TaggerContext) -> ReapTags {
    let lower = build_haystack(ctx);
    if lower.is_empty() {
        return ReapTags::default();
    }

    const PRIMARY: &[&str] = &["reap", "reasoning-as-planning", "reasoning_path"];
    const SECONDARY: &[&str] = &["r1", "qwq", "thinking"];

    if PRIMARY.iter().any(|t| lower.contains(t)) {
        return ReapTags {
            is_reasoning_model: true,
            method: Some("reasoning".to_string()),
        };
    }
    if SECONDARY.iter().any(|t| lower.contains(t)) {
        return ReapTags {
            is_reasoning_model: true,
            method: Some("reasoning".to_string()),
        };
    }

    ReapTags::default()
}

/// Compose the haystack: `(id) + " " + (card_text)`.
fn build_haystack(ctx: &TaggerContext) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !ctx.id.is_empty() {
        parts.push(ctx.id.clone());
    }
    if let Some(card) = ctx.effective_card_text() {
        parts.push(card.to_string());
    }
    parts.join(" ").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_reap_via_id() {
        let ctx = TaggerContext::from_id("phenotype/reap-7b", "phenotype");
        assert!(tag(&ctx).is_reasoning_model);
    }

    #[test]
    fn detect_r1_via_card() {
        let card = "# Model\n\nThis is a DeepSeek R1 distill.\n";
        let ctx = TaggerContext {
            id: "deepseek-ai/DeepSeek-R1-Distill-Qwen-7B".to_string(),
            org: "deepseek-ai".to_string(),
            card_text: Some(card.to_string()),
            ..Default::default()
        };
        assert!(tag(&ctx).is_reasoning_model);
    }

    #[test]
    fn no_signal_is_default() {
        let ctx = TaggerContext::from_id("meta-llama/Llama-3-8B", "meta-llama");
        assert!(!tag(&ctx).is_reasoning_model);
        assert_eq!(tag(&ctx).method, None);
    }
}
