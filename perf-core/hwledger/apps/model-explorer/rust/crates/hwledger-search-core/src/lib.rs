//! `hwledger-search-core` — foundational traits, taxonomy, fusion, and skill
//! registry used by every other search-* crate and the `hwledger-cli`,
//! `hwledger-server`, and `hwledger-mcp` binaries.
//!
//! This crate is dependency-light (serde + thiserror + anyhow) so it can be
//! consumed by anything from a synchronous CLI to an async MCP server without
//! dragging the rest of the workspace along.

#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

// Manual `impl Default` blocks on taxonomy enums (`impl Default for ModelKind
// { fn default() -> Self { Self::Base } }`) are preferred over
// `#[derive(Default)] + #[default]` so the default variant reads as a single,
// self-documenting location next to the rest of the API surface. The clippy
// `derivable_impls` lint is therefore intentionally disabled crate-wide.
#![allow(clippy::derivable_impls)]

pub mod error;
pub mod taxonomy;
pub mod source_adapter;
pub mod fusion;
pub mod query;
pub mod skills;

pub use error::CoreError;
pub use taxonomy::arch::{ArchKind, AttentionKind, MlpKind, RopeVariant};
pub use taxonomy::faceted::Facets;
pub use taxonomy::modality::Modality;
pub use taxonomy::model_kind::ModelKind;
pub use fusion::{rrf_fuse, Scored};
pub use query::{FusedResult, Query};
pub use skills::{SearchContext, SearchIntent, SearchSkill, SkillRegistry};
pub use source_adapter::{CandidateId, RawModel, SourceAdapter};
