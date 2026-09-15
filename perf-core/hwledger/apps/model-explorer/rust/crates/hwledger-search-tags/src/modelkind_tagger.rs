//! Model-kind tagger.
//!
//! Classifies a model into a single [`ModelKind`] variant based on the
//! `pipeline_tag`, `id`, and (optionally) the model card text. The dispatch
//! is keyword-matching on lowercased haystacks; the priority order is the
//! order in [`tag`] — more specific kinds first, then chat/instruct, then
//! the fall-through (`Base`).
//!
//! [`ModelKind`]: hwledger_search_core::ModelKind

use hwledger_search_core::ModelKind;

use crate::tager_context::TaggerContext;

/// Coarse model-kind classification for a single model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModelKindTags {
    /// The primary, single-label kind for the model.
    pub primary_kind: ModelKind,
}

impl Default for ModelKindTags {
    fn default() -> Self {
        Self {
            primary_kind: ModelKind::default(),
        }
    }
}

/// Heuristic classification.
///
/// The haystack is `(pipeline_tag || raw.pipeline_tag) + " " + id + " " +
/// (card_text || raw.card_text)`. We lower-case once and match prefix-free
/// tokens so `-it` doesn't accidentally match `submit`.
pub fn tag(ctx: &TaggerContext) -> ModelKindTags {
    let haystack = build_haystack(ctx);
    let kind = classify(&haystack);
    ModelKindTags { primary_kind: kind }
}

/// Compose the haystack string we run all keyword matches against.
fn build_haystack(ctx: &TaggerContext) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(p) = ctx.effective_pipeline_tag() {
        parts.push(p.to_string());
    }
    if !ctx.id.is_empty() {
        parts.push(ctx.id.clone());
    }
    if let Some(card) = ctx.effective_card_text() {
        parts.push(card.to_string());
    }
    parts.join(" ").to_ascii_lowercase()
}

/// Pure classifier — kept separate so unit tests can hit it without
/// building a `TaggerContext`.
fn classify(hay: &str) -> ModelKind {
    // Embeddings / rerankers / audio / vision — checked first because they
    // typically also match more generic patterns downstream.
    if has_token(hay, &["embed", "bge", "e5", "gte", "sentence"]) {
        return ModelKind::Embedding;
    }
    if has_token(hay, &["rerank", "cross-encoder"]) {
        return ModelKind::Reranker;
    }
    if has_token(hay, &["whisper", "wav2vec", "wav2vec2", "audio", "tts"]) {
        return ModelKind::Audio;
    }
    if has_token(hay, &["vit", "clip", "vision-encoder", "siglip"]) {
        return ModelKind::VisionEncoder;
    }
    if has_token(hay, &["vlm", "vision-language", "llava", "qwen-vl", "vl-", " qvl"]) {
        return ModelKind::VisionLanguage;
    }

    // Task-specific specialisations.
    if has_token(hay, &["coder", "code-", "codestral", "deepseek-coder", "qwen-coder"]) {
        return ModelKind::Coding;
    }
    if has_token(hay, &["reason", "r1", "qwq", "reasoning-as-planning", "reasoning_path"]) {
        return ModelKind::Reasoning;
    }
    if has_token(hay, &["agent", "tool", "hermes", "tooluse"]) {
        return ModelKind::Agentic;
    }

    // Chat / Instruct — priority: Chat wins when both `chat`/`-it`/`-chat`
    // and `instruct` are present. Per the spec test, "Instruct" alone
    // (e.g. `Llama-3.1-8B-Instruct`) also maps to Chat.
    if has_token(hay, &["chat", "-chat", "-it", "instruct"]) {
        return ModelKind::Chat;
    }

    // Provenance-ish kinds.
    if has_token(hay, &["merge"]) {
        return ModelKind::Merge;
    }
    if has_token(hay, &["lora", "adapter"]) {
        return ModelKind::Adapter;
    }

    ModelKind::Base
}

/// `true` if any token in `tokens` appears as a substring of `hay`.
/// We use substring matching (not word-boundary) because model ids embed
/// punctuation freely (`-it`, `-chat`, `qwen-vl`).
fn has_token(hay: &str, tokens: &[&str]) -> bool {
    tokens.iter().any(|t| hay.contains(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_chat_over_instruct() {
        // Both "chat" and "instruct" present — Chat wins.
        let kind = classify("meta-llama llama-3-8b instruct chat");
        assert_eq!(kind, ModelKind::Chat);
    }

    #[test]
    fn classify_instruct_maps_to_chat() {
        // Spec: "Llama-3.1-8B-Instruct" → Chat. We mirror that.
        assert_eq!(classify("meta-llama-llama-3.1-8b-instruct"), ModelKind::Chat);
    }

    #[test]
    fn classify_embedding() {
        assert_eq!(classify("BAAI/bge-large-en-v1.5"), ModelKind::Embedding);
    }

    #[test]
    fn classify_coding() {
        assert_eq!(classify("qwen2.5-coder-7b"), ModelKind::Coding);
    }

    #[test]
    fn classify_vision_language() {
        assert_eq!(classify("llava-1.5-7b"), ModelKind::VisionLanguage);
    }

    #[test]
    fn classify_reasoning() {
        assert_eq!(classify("deepseek-r1-distill"), ModelKind::Reasoning);
    }

    #[test]
    fn classify_default_base() {
        assert_eq!(classify("meta-llama/Llama-3-8B"), ModelKind::Base);
    }
}
