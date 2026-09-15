//! Ingest-specific error type.
//!
//! Kept narrow on purpose: anything that bubbles out of the ingest layer
//! reduces to one of these five variants. Convertible into
//! [`hwledger_search_core::CoreError`] via [`IngestError::into_core`].

use thiserror::Error;

/// Errors produced by `hwledger-search-ingest` adapters.
///
/// Variants are intentionally coarse — the underlying adapter (HF,
/// ModelScope, …) is allowed to flatten every transport / serialization
/// failure into one of these categories so the rest of the pipeline never
/// sees a third-party error type.
#[derive(Debug, Error)]
pub enum IngestError {
    /// An HTTP request failed (network error, non-2xx status, etc.).
    /// The string is a short, human-readable summary; do not embed payloads.
    #[error("http error: {0}")]
    Http(String),

    /// `serde_json` failed to (de)serialize something.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A concrete backend (tantivy, lancedb, etc.) rejected the operation.
    #[error("backend error: {0}")]
    Backend(String),

    /// The upstream source rate-limited the caller. Distinct from
    /// [`IngestError::Http`] so the seed builder can decide to back off
    /// instead of hard-failing.
    #[error("rate limited by upstream")]
    RateLimited,

    /// The upstream source asked for credentials (typically a 401/403).
    #[error("authentication required")]
    AuthRequired,
}

impl IngestError {
    /// Convenience constructor for ad-hoc `Http` errors.
    pub fn http<S: Into<String>>(msg: S) -> Self {
        Self::Http(msg.into())
    }

    /// Convenience constructor for ad-hoc `Backend` errors.
    pub fn backend<S: Into<String>>(msg: S) -> Self {
        Self::Backend(msg.into())
    }

    /// Convert into the core error type used by `SourceAdapter::fetch_raw`.
    pub fn into_core(self) -> hwledger_search_core::CoreError {
        match self {
            Self::Json(e) => hwledger_search_core::CoreError::Json(e),
            other => hwledger_search_core::CoreError::Backend(other.to_string()),
        }
    }
}
