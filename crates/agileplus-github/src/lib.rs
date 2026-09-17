//! agileplus-github — GitHub integration via octocrab (read layer)
//! and raw reqwest (sync/write layer).
//!
//! # Modules
//! - `client` — rate-limited reqwest client for create/update/get issues
//! - `sync`   — conflict-aware sync adapter for backlog items
//! - `octo`   — octocrab-based read client (`list_issues`, `list_prs`)

pub mod client;
pub mod map;
pub mod octo;
pub mod sync;

pub use octo::{Error, GitHubClient};
