//! Crate-wide error types.
//!
//! Search-core deliberately keeps errors small and `thiserror`-derived so they
//! can be losslessly converted into `anyhow::Error` (via the blanket `From`
//! impl) without forcing every caller to depend on `thiserror`.

use thiserror::Error;

/// Errors a `SourceAdapter`, a `SearchSkill`, or any other core primitive may
/// surface back to callers.
#[derive(Debug, Error)]
pub enum CoreError {
    /// `serde_json` failed to (de)serialize something.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A concrete backend (tantivy, lancedb, HF hub, etc.) rejected the
    /// operation. The string carries a backend-specific, human-readable
    /// explanation — adapters are expected to NOT include huge payloads here.
    #[error("backend error: {0}")]
    Backend(String),

    /// Caller-supplied arguments were structurally well-formed but semantically
    /// invalid (e.g. `limit == 0`, conflicting facets).
    #[error("invalid arguments: {0}")]
    InvalidArgs(String),

    /// The requested resource — typically a model identified by
    /// `(source, id)` — could not be located in the upstream adapter.
    #[error("not found: {0}")]
    NotFound(String),
}

impl CoreError {
    /// Convenience constructor for ad-hoc `Backend` errors.
    pub fn backend<S: Into<String>>(msg: S) -> Self {
        Self::Backend(msg.into())
    }

    /// Convenience constructor for ad-hoc `InvalidArgs` errors.
    pub fn invalid_args<S: Into<String>>(msg: S) -> Self {
        Self::InvalidArgs(msg.into())
    }

    /// Convenience constructor for ad-hoc `NotFound` errors.
    pub fn not_found<S: Into<String>>(msg: S) -> Self {
        Self::NotFound(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_serde_json_error() {
        // Round-trip a deliberately-bad JSON string to provoke an error.
        let bad: Result<serde_json::Value, _> = serde_json::from_str("{not json");
        let err = bad.expect_err("must fail");
        let core = CoreError::from(err);
        match core {
            CoreError::Json(_) => {}
            other => panic!("expected Json variant, got {other:?}"),
        }
    }

    #[test]
    fn convenience_constructors() {
        let b = CoreError::backend("tantivy crashed");
        let a = CoreError::invalid_args("limit must be > 0");
        let n = CoreError::not_found("hf:org/missing");
        assert!(matches!(b, CoreError::Backend(_)));
        assert!(matches!(a, CoreError::InvalidArgs(_)));
        assert!(matches!(n, CoreError::NotFound(_)));
    }
}
