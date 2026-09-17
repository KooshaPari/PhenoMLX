// SPDX-License-Identifier: MIT OR Apache-2.0
//! agileplus-witness — MEOW witness primitive.
//!
//! A witness provides evidence and a verdict (Pass, Fail, Abstain) for a
//! bead within a convoy. The [`VerdictEngine`] aggregates witness votes
//! and drives the bead to `Completed` or `Failed`.
//!
//! Traceability: audit recs #16 (MEOW witness primitive)

pub mod evidence;
pub mod verdict;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use evidence::Evidence;
use verdict::Verdict;

/// A witness record for a single bead.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Witness {
    pub id: String,
    pub bead_id: Uuid,
    pub verdict: Verdict,
    pub evidence: Vec<Evidence>,
    pub signed_by: String,
}

impl Witness {
    /// Create a new witness.
    pub fn new(
        id: impl Into<String>,
        bead_id: Uuid,
        verdict: Verdict,
        evidence: Vec<Evidence>,
        signed_by: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            bead_id,
            verdict,
            evidence,
            signed_by: signed_by.into(),
        }
    }
}
