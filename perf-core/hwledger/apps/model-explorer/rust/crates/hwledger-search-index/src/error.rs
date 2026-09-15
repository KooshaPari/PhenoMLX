//! Error types for the `hwledger-search-index` crate.

use thiserror::Error;

/// All errors that can be returned from this crate.
#[derive(Debug, Error)]
pub enum IndexError {
    /// Wraps an error returned by the underlying Tantivy engine.
    #[error("tantivy error: {0}")]
    Tantivy(String),

    /// Wraps an error returned by the underlying LanceDB engine.
    ///
    /// Only present when the `lancedb` feature is enabled — without the
    /// feature there is no LanceDB to wrap, so the variant is omitted
    /// from the enum entirely. This keeps the default BM25-only build
    /// from dragging in `lancedb` (and its arrow stack) just for a
    /// dead-code error variant.
    #[cfg(feature = "lancedb")]
    #[error("lancedb error: {0}")]
    Lance(String),

    /// Filesystem I/O failed (e.g. creating the index directory).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A caller supplied invalid arguments (e.g. empty `id`).
    #[error("invalid arguments: {0}")]
    InvalidArgs(String),
}

impl From<tantivy::TantivyError> for IndexError {
    fn from(value: tantivy::TantivyError) -> Self {
        Self::Tantivy(value.to_string())
    }
}

impl From<anyhow::Error> for IndexError {
    fn from(value: anyhow::Error) -> Self {
        Self::Tantivy(value.to_string())
    }
}

#[cfg(feature = "lancedb")]
impl From<lancedb::Error> for IndexError {
    fn from(value: lancedb::Error) -> Self {
        Self::Lance(value.to_string())
    }
}