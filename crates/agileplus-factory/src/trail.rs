// SPDX-License-Identifier: MIT OR Apache-2.0
//! Action trail — every worker step logged with outcome.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single action a worker can perform.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    ReadFile { path: String },
    WriteFile { path: String, content: String },
    RunTest { command: String },
    GitCommit { message: String },
    GitPush { branch: String },
    OpenPr { branch: String, title: String },
}

/// One row in the trail.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrailEntry {
    pub timestamp: DateTime<Utc>,
    pub action: Action,
    pub outcome: Result<String, String>,
}

/// Per-worker action log.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Trail {
    pub entries: Vec<TrailEntry>,
}

impl Trail {
    pub fn new() -> Self {
        Self::default()
    }

    /// Log an action with its outcome.
    pub fn log(&mut self, action: Action, outcome: Result<String, String>) {
        self.entries.push(TrailEntry {
            timestamp: Utc::now(),
            action,
            outcome,
        });
    }

    /// Serialize the trail to JSON.
    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(&self.entries)?)
    }

    /// All entries in chronological order.
    pub fn entries(&self) -> &[TrailEntry] {
        &self.entries
    }

    /// Count of successful entries.
    pub fn success_count(&self) -> usize {
        self.entries.iter().filter(|e| e.outcome.is_ok()).count()
    }

    /// Count of failed entries.
    pub fn failure_count(&self) -> usize {
        self.entries.iter().filter(|e| e.outcome.is_err()).count()
    }
}
