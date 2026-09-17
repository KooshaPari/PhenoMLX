// SPDX-License-Identifier: MIT OR Apache-2.0
//! Factory configuration — autonomy levels, repo, label filter, worker limits.

use serde::{Deserialize, Serialize};

/// How autonomous the factory is allowed to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    /// Suggest changes only — open a draft PR for human review.
    Suggest,
    /// Execute the full pipeline — open a real PR.
    #[default]
    Execute,
    /// Execute and auto-merge after CI passes.
    Merge,
}

/// Configuration for the dark-factory loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryConfig {
    /// Repository to watch, in `owner/repo` format.
    pub repo: String,
    /// Issue label that triggers the factory (default: `"factory"`).
    #[serde(default = "default_label_filter")]
    pub label_filter: String,
    /// Maximum concurrent workers.
    #[serde(default = "default_max_workers")]
    pub max_workers: usize,
    /// Seconds between queue polls.
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    /// Autonomy level.
    #[serde(default)]
    pub autonomy_level: AutonomyLevel,
    /// GitHub personal-access token (or app token) for API calls.
    pub github_token: String,
}

impl FactoryConfig {
    /// Build with a token and repo; everything else defaults.
    pub fn new(repo: impl Into<String>, github_token: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            label_filter: default_label_filter(),
            max_workers: default_max_workers(),
            poll_interval_secs: default_poll_interval_secs(),
            autonomy_level: AutonomyLevel::default(),
            github_token: github_token.into(),
        }
    }
}

fn default_label_filter() -> String {
    "factory".into()
}

fn default_max_workers() -> usize {
    4
}

fn default_poll_interval_secs() -> u64 {
    60
}
