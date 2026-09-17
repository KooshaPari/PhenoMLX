// SPDX-License-Identifier: MIT OR Apache-2.0
//! agileplus-convoy — MEOW convoy primitive.
//!
//! A convoy is a two-phase commit container for beads. Each bead wraps a
//! [`agileplus_triage::claim::Claim`] and tracks its own lifecycle state.
//! The [`Coordinator`] drives Phase-1 (Prepare) and Phase-2 (Commit / Abort).
//!
//! Traceability: audit recs #14 (MEOW convoy primitive)

pub mod bead;
pub mod coordinator;
pub mod store;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use bead::Bead;

/// Lifecycle of a convoy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvoyStatus {
    /// Accepting new beads.
    Open,
    /// No new beads; waiting for coordinator decision.
    Closed,
    /// All beads completed and claims released.
    Committed,
    /// At least one bead failed or timeout elapsed.
    Aborted,
}

/// A convoy is a two-phase commit transaction container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Convoy {
    pub id: Uuid,
    pub beads: Vec<Bead>,
    pub coordinator: String,
    pub status: ConvoyStatus,
    pub timeout: DateTime<Utc>,
}

impl Convoy {
    /// Create a new open convoy.
    pub fn new(coordinator: impl Into<String>, timeout: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            beads: Vec::new(),
            coordinator: coordinator.into(),
            status: ConvoyStatus::Open,
            timeout,
        }
    }

    /// Add a bead while the convoy is open.
    pub fn add_bead(&mut self, bead: Bead) {
        if self.status == ConvoyStatus::Open {
            self.beads.push(bead);
        }
    }

    /// Close the convoy to new beads.
    pub fn close(&mut self) {
        self.status = ConvoyStatus::Closed;
    }
}
