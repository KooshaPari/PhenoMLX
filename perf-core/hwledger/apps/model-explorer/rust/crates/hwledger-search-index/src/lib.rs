//! Tantivy BM25 store + collapse rule + hybrid query driver.
//!
//! This crate is the *search* half of `hwLedger`'s model-explorer pipeline:
//!
//! - [`tantivy_store::TantivyStore`] — persistent BM25 index backed by
//!   Tantivy, with `id` as the primary key and per-field boosts
//!   (name^3, org^2, kind^2, family^2, arch^1, quants^1, card_snippet^1).
//! - [`lance_store::LanceStore`] — feature-gated dense-ANN (approximate
//!   nearest neighbor) over LanceDB. **Only present when the `lancedb`
//!   cargo feature is enabled**; without it the module is `cfg`'d out
//!   entirely and the default BM25-only build doesn't link `lancedb`.
//! - [`collapse::CollapseRule`] / [`collapse::collapse_variants`] /
//!   [`collapse::collapse_key`] — collapse BM25 hits that share a quantized
//!   base id into one "model family" row.
//! - [`ingest::IndexedModel`] / [`ingest::upsert_model`] — the typed
//!   transport shape that upstream ingesters use to push one row into the
//!   store.
//! - [`query::run_hybrid`] — the public entry point other crates call.
//!   v1 (default features) is BM25-only; when the `lancedb` feature is
//!   enabled the function gains an extra `Option<&LanceStore>` + `&[f32]`
//!   argument pair and fuses BM25 + ANN with RRF (`k = 60`). The OFF
//!   signature is stable so callers can transition to fused results
//!   without API churn.
//!
//! ## Feature split
//!
//! The `lancedb` feature is **default OFF** (`default = []`). The
//! `lancedb` crate is an *optional* dependency
//! (`lancedb = { workspace = true, optional = true }`) and is only linked
//! when the feature is requested. `lance_store` reaches its arrow types
//! via `lancedb::arrow::*` re-exports, so the feature pulls in only
//! `lancedb` itself — the dependency-light CLI / server / MCP front-ends
//! avoid dragging the entire vector-store stack along when their
//! operators don't need dense recall.
//!
//! Other crates import:
//!
//! ```ignore
//! use hwledger_search_index::{
//!     TantivyStore, IndexedDoc, IndexHit, IndexError,
//!     IndexedModel, upsert_model,
//!     CollapseRule, collapse_variants, collapse_key,
//!     run_hybrid,
//! };
//! ```
//!
//! When the `lancedb` feature is on, also available:
//!
//! ```ignore
//! use hwledger_search_index::{
//!     LanceStore, EMBEDDINGS_TABLE, ID_COLUMN, VEC_COLUMN,
//! };
//! ```

mod collapse;
mod error;
mod ingest;
mod lance_ann;
mod query;
mod tantivy_store;

#[cfg(feature = "lancedb")]
mod lance_store;

pub use collapse::{collapse_key, collapse_variants, CollapseRule, CollapsedHit};
pub use error::IndexError;
pub use ingest::{upsert_model, IndexedModel};
pub use lance_ann::{AnnRow, LanceAnn};
pub use query::{run_hybrid, IndexHit};
pub use tantivy_store::{IndexedDoc, TantivyStore};

#[cfg(feature = "lancedb")]
pub use lance_store::{EmbeddingRow, LanceStore, EMBEDDINGS_TABLE, ID_COLUMN, VEC_COLUMN};