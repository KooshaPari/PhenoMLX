//! `hwledger-search-evals` — extractors that turn a Hugging Face-style
//! model card into structured `EvalRecord` / `CardRow` / `ReadmeEval`
//! rows.
//!
//! Three independent surfaces live here:
//! * [`model_index`] parses the `model-index` YAML block.
//! * [`card_table`]   extracts numeric benchmark rows from markdown tables.
//! * [`readme_results`] mines "Results" / "Benchmark" / "Evaluation"
//!   sections of a README for inline scores.
//!
//! All three are pure functions over `&str` → `Vec<…>`, so they're trivially
//! usable from `search-index`, `hwledger-cli`, or unit tests.

#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

pub mod card_table;
pub mod error;
pub mod model_index;
pub mod readme_results;

pub use card_table::{parse_card_table, CardRow};
pub use error::EvalError;
pub use model_index::{parse_model_index, EvalRecord};
pub use readme_results::{parse_readme_results, ReadmeEval};