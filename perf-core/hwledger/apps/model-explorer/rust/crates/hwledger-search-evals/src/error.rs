//! Crate-wide error type for the eval extractors.

use thiserror::Error;

/// Failures returned by [`crate::model_index`], [`crate::card_table`],
/// and [`crate::readme_results`].
#[derive(Debug, Error)]
pub enum EvalError {
    /// Underlying serializer (e.g. `serde_yaml`) rejected the input.
    #[error("parse error: {0}")]
    Parse(String),

    /// I/O failure (only relevant when the crate grows adapters that read
    /// from disk in the future).
    #[error("io error: {0}")]
    Io(String),
}

impl From<serde_yaml::Error> for EvalError {
    fn from(e: serde_yaml::Error) -> Self {
        Self::Parse(e.to_string())
    }
}

impl From<serde_json::Error> for EvalError {
    fn from(e: serde_json::Error) -> Self {
        Self::Parse(e.to_string())
    }
}

impl From<std::io::Error> for EvalError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}