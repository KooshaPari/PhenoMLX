// SPDX-License-Identifier: MIT OR Apache-2.0
//! Two-phase commit coordinator for convoys.

use anyhow::{anyhow, Result};
use chrono::Utc;

use agileplus_triage::claim::ClaimStoreTrait;

use crate::{bead::BeadState, Convoy, ConvoyStatus};

/// Coordinator drives the two-phase commit protocol on a convoy.
#[derive(Debug, Default)]
pub struct Coordinator;

impl Coordinator {
    /// Phase 1: Prepare — returns `true` if all beads are `Completed`.
    pub fn prepare(convoy: &Convoy) -> bool {
        if convoy.beads.is_empty() {
            return false;
        }
        convoy.beads.iter().all(|b| b.state == BeadState::Completed)
    }

    /// Phase 2: Commit — release all claims and mark convoy as committed.
    pub fn commit(convoy: &mut Convoy, store: &mut dyn ClaimStoreTrait) -> Result<()> {
        if !Self::prepare(convoy) {
            return Err(anyhow!("prepare phase failed: not all beads completed"));
        }
        for bead in &convoy.beads {
            store.release(&bead.claim.id);
        }
        convoy.status = ConvoyStatus::Committed;
        Ok(())
    }

    /// Abort — release all claims and mark convoy as aborted.
    ///
    /// Triggered when any bead is `Failed` or the convoy has timed out.
    pub fn abort(convoy: &mut Convoy, store: &mut dyn ClaimStoreTrait) -> Result<()> {
        for bead in &convoy.beads {
            store.release(&bead.claim.id);
        }
        convoy.status = ConvoyStatus::Aborted;
        Ok(())
    }

    /// Check whether the convoy has timed out.
    pub fn is_timed_out(convoy: &Convoy) -> bool {
        Utc::now() > convoy.timeout
    }

    /// Auto-evaluate: commit if all completed, abort if any failed or timed out.
    pub fn evaluate(convoy: &mut Convoy, store: &mut dyn ClaimStoreTrait) -> Result<ConvoyStatus> {
        if convoy.beads.iter().any(|b| b.state == BeadState::Failed) || Self::is_timed_out(convoy) {
            Self::abort(convoy, store)?;
            return Ok(ConvoyStatus::Aborted);
        }
        if Self::prepare(convoy) {
            Self::commit(convoy, store)?;
            return Ok(ConvoyStatus::Committed);
        }
        Ok(convoy.status)
    }
}
