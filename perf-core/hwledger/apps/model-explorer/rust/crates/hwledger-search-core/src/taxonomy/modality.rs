//! Input/output modality classification.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Modalities a model can accept or produce.
///
/// `Default::default() == Modality::Text` reflects the empirical fact that the
/// overwhelming majority of LLMs in the wild are text-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    /// Plain natural-language text.
    Text,
    /// Programming-language source code.
    Code,
    /// Still images / video frames as input.
    Vision,
    /// Audio waveforms (ASR, TTS).
    Audio,
    /// Image generation (Stable Diffusion, FLUX, etc.).
    ImageGen,
    /// Any model that natively consumes more than one modality.
    Multimodal,
}

impl Default for Modality {
    fn default() -> Self {
        Self::Text
    }
}

impl fmt::Display for Modality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Text => "text",
            Self::Code => "code",
            Self::Vision => "vision",
            Self::Audio => "audio",
            Self::ImageGen => "image_gen",
            Self::Multimodal => "multimodal",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_text() {
        assert_eq!(Modality::default(), Modality::Text);
    }

    #[test]
    fn round_trip() {
        for m in [
            Modality::Text,
            Modality::Code,
            Modality::Vision,
            Modality::Audio,
            Modality::ImageGen,
            Modality::Multimodal,
        ] {
            let j = serde_json::to_string(&m).unwrap();
            let back: Modality = serde_json::from_str(&j).unwrap();
            assert_eq!(back, m);
        }
    }
}
