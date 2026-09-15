//! `hwledger-search-rag` — chunking, deterministic stub embedding, and
//! cosine-similarity retrieval over [`Chunk`]s.
//!
//! This crate is intentionally self-contained: it only depends on
//! `hwledger-search-core` for the workspace-level `serde` / `thiserror`
//! conventions. Real embedding backends (FastEmbed, candle, …) can plug in
//! by implementing the [`Embedder`] trait.

#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

pub mod chunker;
pub mod embedder;
pub mod error;
pub mod rag;

pub use chunker::{Chunk, Chunker};
pub use embedder::{Embedder, EmbedderConfig, EmbedderImpl, Qwen3Embedder, StubEmbedder};
pub use error::RagError;
pub use rag::{retrieve, RagResult, RAGConfig};