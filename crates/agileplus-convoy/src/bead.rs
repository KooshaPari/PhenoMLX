// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bead — a single unit of work within a convoy.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use agileplus_triage::claim::Claim;

/// State of a bead within the convoy lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeadState {
    /// Waiting to be claimed.
    Pending,
    /// Claim has been issued.
    Claimed,
    /// Work is actively happening.
    InProgress,
    /// Work finished successfully.
    Completed,
    /// Work failed.
    Failed,
}

/// A bead wraps an existing claim and tracks its execution state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bead {
    pub id: Uuid,
    pub claim: Claim,
    pub state: BeadState,
    pub payload: serde_json::Value,
    pub owner: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Bead {
    /// Create a new pending bead.
    pub fn new(claim: Claim, payload: serde_json::Value, owner: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            claim,
            state: BeadState::Pending,
            payload,
            owner: owner.into(),
            created_at: now,
            completed_at: None,
        }
    }

    /// Mark the bead as in-progress.
    pub fn start(&mut self) {
        self.state = BeadState::InProgress;
    }

    /// Mark the bead as completed.
    pub fn complete(&mut self) {
        self.state = BeadState::Completed;
        self.completed_at = Some(Utc::now());
    }

    /// Mark the bead as failed.
    pub fn fail(&mut self) {
        self.state = BeadState::Failed;
        self.completed_at = Some(Utc::now());
    }
}
