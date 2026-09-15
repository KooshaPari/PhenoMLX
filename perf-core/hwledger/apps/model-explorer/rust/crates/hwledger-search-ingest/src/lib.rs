//! `hwledger-search-ingest` — upstream source adapters (`HuggingFace`,
//! …) plus the seed-index builder, lazy-populate cache, and
//! neighborhood-expansion helpers that feed the rest of the search
//! pipeline.
//!
//! Public surface is intentionally small — see the re-exports below.
//! Adapter authors implement [`hwledger_search_core::SourceAdapter`]
//! and plug into [`seed_builder::build_seed_index`] /
//! [`lazy_populate::lazy_populate`] without touching the rest of the
//! workspace.

#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

pub mod config_parser;
pub mod error;
pub mod expansion;
pub mod huggingface;
pub mod lazy_populate;
pub mod models_dev;
pub mod rate_limit;
pub mod seed_builder;
pub mod tree_parser;

pub use config_parser::{parse_config_value, ParsedConfig};
pub use error::IngestError;
pub use expansion::{expand_neighborhood, ExpansionConfig};
pub use huggingface::HuggingFaceAdapter;
pub use lazy_populate::{lazy_populate, PopulateGate};
pub use seed_builder::{build_seed_index, SeedBuild, SeedReport, SeedSink};
pub use tree_parser::{parse_tree_value, TreeEntry};
