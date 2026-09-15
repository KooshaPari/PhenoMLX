//! Errors raised by the chunker, embedder, and retrieval pipeline.

use thiserror::Error;

/// Failure modes for [`crate::chunker`], [`crate::embedder`], and
/// [`crate::rag`].
#[derive(Debug, Error)]
pub enum RagError {
    /// The underlying embedder rejected the input (e.g. tokenizer error).
    #[error("embedder error: {0}")]
    Embedder(String),

    /// The caller asked for retrieval with an empty query string.
    #[error("query is empty")]
    EmptyQuery,

    /// An embedder produced a vector of a different dimensionality than the
    /// one previously observed.
    #[error("embedding dimension mismatch: expected {expected}, got {actual}")]
    DimMismatch {
        /// The dimensionality the caller has been working with.
        expected: usize,
        /// The dimensionality the offending vector actually had.
        actual: usize,
    },
}