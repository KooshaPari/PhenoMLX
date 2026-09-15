//! Canonical `ModelKind` taxonomy for ingested model cards.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Coarse classification of a model's intended use / training objective.
///
/// The ordering of variants is meaningful: when serialized to JSON `Base` is
/// the default and is persisted as an explicit tag so downstream tooling
/// doesn't have to special-case missing values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// Unmodified base / foundation model — no instruction tuning.
    Base,
    /// Instruction-tuned variant of a base model.
    Instruct,
    /// Multi-turn chat variant (OpenChat, OpenAssistant style).
    Chat,
    /// Reinforcement / chain-of-thought reasoning variant (o1, R1 style).
    Reasoning,
    /// Code-focused variant (CodeLlama, DeepSeek-Coder style).
    Coding,
    /// Tool-using / agentic variant (Gorilla, Toolformer style).
    Agentic,
    /// Embedding model — produces dense vector representations.
    Embedding,
    /// Cross-encoder / LLM-based reranker.
    Reranker,
    /// Vision-language model (image + text → text).
    VisionLanguage,
    /// Vision encoder only (CLIP-ViT, SigLIP style).
    VisionEncoder,
    /// Audio model (ASR, TTS, audio understanding).
    Audio,
    /// Model merge (e.g. SLERP, TIES, DARE).
    Merge,
    /// Standalone fine-tune delta weights / patch.
    Finetune,
    /// LoRA / adapter / PEFT delta over an existing base.
    Adapter,
    /// Standalone quantized artifact (GGUF, AWQ, GPTQ, etc.).
    Quant,
}

impl Default for ModelKind {
    fn default() -> Self {
        Self::Base
    }
}

impl fmt::Display for ModelKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Base => "base",
            Self::Instruct => "instruct",
            Self::Chat => "chat",
            Self::Reasoning => "reasoning",
            Self::Coding => "coding",
            Self::Agentic => "agentic",
            Self::Embedding => "embedding",
            Self::Reranker => "reranker",
            Self::VisionLanguage => "vision_language",
            Self::VisionEncoder => "vision_encoder",
            Self::Audio => "audio",
            Self::Merge => "merge",
            Self::Finetune => "finetune",
            Self::Adapter => "adapter",
            Self::Quant => "quant",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_base() {
        assert_eq!(ModelKind::default(), ModelKind::Base);
    }

    #[test]
    fn display_matches_serde_rename_all() {
        for k in [
            ModelKind::Base,
            ModelKind::Instruct,
            ModelKind::Chat,
            ModelKind::Reasoning,
            ModelKind::Coding,
            ModelKind::Agentic,
            ModelKind::Embedding,
            ModelKind::Reranker,
            ModelKind::VisionLanguage,
            ModelKind::VisionEncoder,
            ModelKind::Audio,
            ModelKind::Merge,
            ModelKind::Finetune,
            ModelKind::Adapter,
            ModelKind::Quant,
        ] {
            let j = serde_json::to_string(&k).unwrap();
            // serde_json::to_string produces `"instruct"` style.
            let inner = j.trim_matches('"');
            assert_eq!(inner, k.to_string());
        }
    }

    #[test]
    fn round_trip() {
        for k in [
            ModelKind::Base,
            ModelKind::Reasoning,
            ModelKind::VisionLanguage,
            ModelKind::Quant,
        ] {
            let j = serde_json::to_string(&k).unwrap();
            let back: ModelKind = serde_json::from_str(&j).unwrap();
            assert_eq!(back, k);
        }
    }
}
