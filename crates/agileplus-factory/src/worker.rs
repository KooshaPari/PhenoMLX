// SPDX-License-Identifier: MIT OR Apache-2.0
//! Worker struct — id, claim, issue binding, TTL heartbeat.

use anyhow::Result;

use agileplus_triage::claim::{ClaimKind, ClaimReason, ClaimStoreTrait};

use crate::trail::Trail;

/// A factory worker bound to a single issue + claim.
#[derive(Debug, Clone)]
pub struct Worker {
    pub id: String,
    pub claim_id: String,
    pub issue_id: u64,
    pub ttl_seconds: i64,
    pub trail: Trail,
}

impl Worker {
    /// Create a new worker. Does **not** heartbeat yet — the caller
    /// must call [`Worker::heartbeat`] after claiming.
    pub fn new(
        id: impl Into<String>,
        claim_id: impl Into<String>,
        issue_id: u64,
        ttl_seconds: i64,
    ) -> Self {
        Self {
            id: id.into(),
            claim_id: claim_id.into(),
            issue_id,
            ttl_seconds,
            trail: Trail::new(),
        }
    }

    /// Refresh the claim's heartbeat in the store.
    pub fn heartbeat<S: ClaimStoreTrait>(&self, store: &mut S) -> bool {
        store.heartbeat(&self.claim_id)
    }

    /// Claim a worktree resource for this worker.
    ///
    /// The `resource` is the canonical branch name,
    /// e.g. `feat/<feature_slug>/<wp_id>`.
    pub fn claim_worktree<S: ClaimStoreTrait>(
        &self,
        store: &mut S,
        resource: &str,
        agent: &str,
    ) -> Option<agileplus_triage::claim::Claim> {
        store.claim(
            &self.claim_id,
            resource,
            ClaimKind::Worktree,
            agent,
            self.ttl_seconds,
            ClaimReason::WipRun(self.id.clone()),
        )
    }

    /// Placeholder agent loop — logs a trail entry and returns.
    /// The real agent loop (read code -> plan edits -> LLM calls)
    /// is a future phase.
    pub async fn run_placeholder(&mut self) -> Result<()> {
        self.trail.log(
            crate::trail::Action::ReadFile {
                path: "README.md".into(),
            },
            Ok("placeholder read".into()),
        );
        self.trail.log(
            crate::trail::Action::RunTest {
                command: "cargo test".into(),
            },
            Ok("placeholder test pass".into()),
        );
        self.trail.log(
            crate::trail::Action::GitCommit {
                message: "factory: placeholder commit".into(),
            },
            Ok("placeholder commit".into()),
        );
        Ok(())
    }
}
