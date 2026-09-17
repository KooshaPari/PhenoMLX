// SPDX-License-Identifier: MIT OR Apache-2.0
//! Verdict engine — aggregates witness votes and decides bead fate.

use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use agileplus_convoy::Convoy;

use crate::Witness;

/// Outcome of an individual witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Pass,
    Fail,
    Abstain,
}

/// Aggregates witness votes and applies the result to beads.
#[derive(Debug, Default)]
pub struct VerdictEngine;

impl VerdictEngine {
    /// Evaluate a convoy's witnesses and update bead states.
    ///
    /// Rules:
    /// - Majority `Pass` → bead becomes `Completed`.
    /// - Majority `Fail` → bead becomes `Failed`.
    /// - Tie or only `Abstain` → no change.
    pub fn evaluate(&self, convoy: &mut Convoy, witnesses: &[Witness]) -> Result<Vec<Uuid>> {
        let mut by_bead: HashMap<Uuid, (usize, usize, usize)> = HashMap::new();
        for w in witnesses {
            let entry = by_bead.entry(w.bead_id).or_insert((0, 0, 0));
            match w.verdict {
                Verdict::Pass => entry.0 += 1,
                Verdict::Fail => entry.1 += 1,
                Verdict::Abstain => entry.2 += 1,
            }
        }

        let mut changed = Vec::new();
        for bead in &mut convoy.beads {
            if let Some(&(pass, fail, abstain)) = by_bead.get(&bead.id) {
                let total = pass + fail + abstain;
                if total == 0 {
                    continue;
                }
                let pass_pct = pass as f64 / total as f64;
                let fail_pct = fail as f64 / total as f64;

                if pass_pct > 0.5 && pass > fail {
                    bead.complete();
                    changed.push(bead.id);
                    info!("bead {} completed by witness majority", bead.id);
                } else if fail_pct > 0.5 && fail > pass {
                    bead.fail();
                    changed.push(bead.id);
                    info!("bead {} failed by witness majority", bead.id);
                }
            }
        }
        Ok(changed)
    }
}
