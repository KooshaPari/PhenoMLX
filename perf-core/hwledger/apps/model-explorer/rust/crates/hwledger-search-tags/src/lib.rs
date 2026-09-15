//! `hwledger-search-tags` — heuristic taggers + a use-case-fit composite
//! orchestrator sitting on top of [`hwledger_search_core`].
//!
//! This crate takes the lossy `RawModel` view produced by adapters
//! (HF, ModelScope, OCI, …) and turns it into a uniform `AllTags` bundle
//! of [`ArchTags`], [`MoeTags`], [`QuantTags`], [`ParamTags`],
//! [`LicenseTags`], [`ModelKindTags`], [`ReapTags`], [`ProvenanceTags`]
//! and [`FitScore`] each consumed by downstream crates (the indexer, the
//! evaluation harness, the server / MCP faceted query layer).
//!
//! The orchestrator's [`tag_all`] is the single entry-point; individual
//! taggers are exported so tests and downstream callers can hit a single
//! axis in isolation.
//!
//! ## Module layout
//!
//! - [`tager_context`] — the shared input bundle every tagger consumes.
//! - [`arch_tagger`]   — architecture family (attention / MLP / RoPE).
//! - [`moe_tagger`]    — mixture-of-experts detection.
//! - [`quant_tagger`]  — quantization / file-format classification.
//! - [`param_tagger`]  — parameter count + bucket.
//! - [`license_tagger`]— license classification + restrictive flag.
//! - [`modelkind_tagger`] — coarse `ModelKind`.
//! - [`reap_tagger`]   — ReAp-reasoning classification.
//! - [`provenance_tagger`] — provenance (original / finetune / merge).
//! - [`usecase_fit_tagger`] — composite `agentic` / `coding` fit score.
//! - [`orchestrator`]  — the top-level `tag_all` entrypoint.
//!
//! Downstream consumers typically only depend on the orchestrator +
//! [`tager_context::TaggerContext`]:
//!
//! ```
//! use hwledger_search_tags::orchestrator::tag_all;
//! use hwledger_search_tags::tager_context::TaggerContext;
//!
//! let ctx = TaggerContext::from_id("meta-llama/Llama-3.1-8B", "meta-llama");
//! let tags = tag_all(&ctx);
//! assert_eq!(tags.provenance.provenance, "original");
//! ```

#![deny(missing_docs)]
#![deny(rust_2018_idioms)]
#![allow(clippy::derivable_impls)]

pub mod arch_tagger;
pub mod license_tagger;
pub mod modelkind_tagger;
pub mod moe_tagger;
pub mod orchestrator;
pub mod param_tagger;
pub mod provenance_tagger;
pub mod quant_tagger;
pub mod reap_tagger;
pub mod tager_context;
pub mod usecase_fit_tagger;

pub use orchestrator::{tag_all, AllTags};
pub use tager_context::TaggerContext;

pub use arch_tagger::{tag as arch_tag, ArchTags};
pub use license_tagger::{tag as license_tag, LicenseTags};
pub use modelkind_tagger::{tag as modelkind_tag, ModelKindTags};
pub use moe_tagger::{tag as moe_tag, MoeTags};
pub use param_tagger::{tag as param_tag, ParamTags};
pub use provenance_tagger::{tag as provenance_tag, ProvenanceTags};
pub use quant_tagger::{tag as quant_tag, QuantTags};
pub use reap_tagger::{tag as reap_tag, ReapTags};
pub use usecase_fit_tagger::{tag as usecase_fit_tag, FitScore};
