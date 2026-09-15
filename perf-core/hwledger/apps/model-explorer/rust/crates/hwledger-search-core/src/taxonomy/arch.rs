//! Architecture taxonomy: token-mixing strategy, attention flavor, MLP type,
//! and RoPE positional-encoding variant.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Overall token-mixing strategy of the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchKind {
    /// Standard fully-dense transformer (every layer sees every token).
    Dense,
    /// Sparse mixture-of-experts transformer (Switch, Mixtral, DeepSeek-MoE).
    #[serde(rename = "moe")]
    Moe,
    /// Hybrid architecture mixing attention with a non-attention sequence
    /// mixer (Jamba, RecurrentGemma, etc.).
    Hybrid,
}

impl Default for ArchKind {
    fn default() -> Self {
        Self::Dense
    }
}

impl fmt::Display for ArchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Dense => "dense",
            Self::Moe => "moe",
            Self::Hybrid => "hybrid",
        };
        f.write_str(s)
    }
}

/// Attention flavor used by the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionKind {
    /// Multi-head attention (the original Vaswani et al. formulation).
    Mha,
    /// Grouped-query attention (GQA) — fewer KV heads than Q heads.
    Gqa,
    /// Multi-query attention (MQA) — a single shared KV head.
    Mqa,
    /// Multi-head latent attention (DeepSeek-V2 / V3).
    Mla,
    /// Sliding-window / local attention (Mistral).
    Sliding,
    /// State-space model block (Mamba) — not strictly attention.
    Ssm,
    /// Hybrid attention combining more than one of the above per layer.
    Hybrid,
    /// Attention sinks / streaming-llm style masking.
    Sink,
}

impl Default for AttentionKind {
    fn default() -> Self {
        Self::Mha
    }
}

impl fmt::Display for AttentionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Mha => "mha",
            Self::Gqa => "gqa",
            Self::Mqa => "mqa",
            Self::Mla => "mla",
            Self::Sliding => "sliding",
            Self::Ssm => "ssm",
            Self::Hybrid => "hybrid",
            Self::Sink => "sink",
        };
        f.write_str(s)
    }
}

/// MLP block type (i.e. what sits between the two linear projections of each
/// transformer block, in addition to GLU gates / activation functions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MlpKind {
    /// Standard two-layer MLP with a squared ReLU / GeLU / SiLU.
    Standard,
    /// SwiGLU-gated MLP (Llama, Mistral).
    #[serde(rename = "swiglu")]
    SwiGlu,
    /// GeLU-gated MLP (GPT-2, Gemma).
    #[serde(rename = "gelu_glu")]
    GeLu,
    /// Plain multi-layer perceptron with no gating (classic Transformer).
    Mlp,
    /// Sparse / MoE MLP.
    Sparse,
}

impl Default for MlpKind {
    fn default() -> Self {
        Self::Standard
    }
}

impl fmt::Display for MlpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Standard => "standard",
            Self::SwiGlu => "swiglu",
            Self::GeLu => "gelu_glu",
            Self::Mlp => "mlp",
            Self::Sparse => "sparse",
        };
        f.write_str(s)
    }
}

/// Rotary positional-encoding variant. `None` indicates an architecture that
/// does not use RoPE at all (e.g. ALiBi or learned absolute positions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RopeVariant {
    /// No RoPE at all.
    None,
    /// Vanilla RoPE (Su et al. 2021).
    Standard,
    /// YaRN — length-extrapolation-friendly NTK-by-parts RoPE.
    #[serde(rename = "yarn")]
    Yarn,
    /// NTK-aware scaling (bloc97).
    NtkScaled,
    /// Dynamic NTK — context-dependent scaling applied at runtime.
    #[serde(rename = "dynamic_ntk")]
    DynamicNtk,
    /// Llama-3 specific RoPE scaling.
    #[serde(rename = "llama3")]
    Llama3,
}

impl Default for RopeVariant {
    fn default() -> Self {
        Self::None
    }
}

impl fmt::Display for RopeVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::None => "none",
            Self::Standard => "standard",
            Self::Yarn => "yarn",
            Self::NtkScaled => "ntk_scaled",
            Self::DynamicNtk => "dynamic_ntk",
            Self::Llama3 => "llama3",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        assert_eq!(ArchKind::default(), ArchKind::Dense);
        assert_eq!(AttentionKind::default(), AttentionKind::Mha);
        assert_eq!(MlpKind::default(), MlpKind::Standard);
        assert_eq!(RopeVariant::default(), RopeVariant::None);
    }

    #[test]
    fn round_trip() {
        for a in [ArchKind::Dense, ArchKind::Moe, ArchKind::Hybrid] {
            let j = serde_json::to_string(&a).unwrap();
            let back: ArchKind = serde_json::from_str(&j).unwrap();
            assert_eq!(back, a);
        }
        for a in [
            AttentionKind::Mha,
            AttentionKind::Gqa,
            AttentionKind::Mqa,
            AttentionKind::Mla,
            AttentionKind::Sliding,
            AttentionKind::Ssm,
            AttentionKind::Hybrid,
            AttentionKind::Sink,
        ] {
            let j = serde_json::to_string(&a).unwrap();
            let back: AttentionKind = serde_json::from_str(&j).unwrap();
            assert_eq!(back, a);
        }
        for m in [
            MlpKind::Standard,
            MlpKind::SwiGlu,
            MlpKind::GeLu,
            MlpKind::Mlp,
            MlpKind::Sparse,
        ] {
            let j = serde_json::to_string(&m).unwrap();
            let back: MlpKind = serde_json::from_str(&j).unwrap();
            assert_eq!(back, m);
        }
        for r in [
            RopeVariant::None,
            RopeVariant::Standard,
            RopeVariant::Yarn,
            RopeVariant::NtkScaled,
            RopeVariant::DynamicNtk,
            RopeVariant::Llama3,
        ] {
            let j = serde_json::to_string(&r).unwrap();
            let back: RopeVariant = serde_json::from_str(&j).unwrap();
            assert_eq!(back, r);
        }
    }
}
