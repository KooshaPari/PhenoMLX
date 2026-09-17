// SPDX-License-Identifier: MIT OR Apache-2.0
//! Dark-factory loop â€” queue â†’ claim â†’ worktree â†’ trail â†’ PR.
//!
//! The `Factory` struct is the entry point. It owns a [`ClaimStoreTrait`]
//! implementation, polls an [`IssueQueue`], and for each issue:
//!
//! 1. Claims a worktree resource.
//! 2. Creates a claim-bound worktree via [`agileplus_git::claim_bound::ClaimBoundWorktree`].
//! 3. Spawns a [`Worker`] that runs a placeholder agent loop and logs a [`Trail`].
//! 4. Opens a PR via [`GitHubPrClient`] (or logs it in Suggest mode).
//! 5. Releases the claim.
//!
//! The real agent loop (read code â†’ plan edits â†’ LLM calls) is a future phase.
//! This crate only scaffolds the pipeline with real Git/Claim primitives.

use std::path::PathBuf;

use anyhow::Result;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

use agileplus_git::claim_bound::ClaimBoundWorktree;
use agileplus_triage::claim::{ClaimStore, ClaimStoreTrait};

pub mod config;
pub mod pr;
pub mod queue;
pub mod trail;
pub mod worker;

pub use config::{AutonomyLevel, FactoryConfig};
pub use pr::{GitHubPr, GitHubPrClient};
pub use queue::{FakeIssueQueue, GitHubIssueQueue, Issue, IssueQueue};
pub use trail::{Action, Trail, TrailEntry};
pub use worker::Worker;

/// The dark factory.
///
/// Owns a [`ClaimStore`] (in-memory) and orchestrates the
/// queue â†’ claim â†’ worktree â†’ worker â†’ PR loop.
///
/// For multi-process deployments, swap the `ClaimStore` for a
/// [`SqliteClaimStore`](agileplus_triage::claim_store_sqlite::SqliteClaimStore)
/// behind a `Mutex`.
#[derive(Debug)]
pub struct Factory<Q: IssueQueue> {
    pub config: FactoryConfig,
    pub claim_store: ClaimStore,
    pub queue: Q,
    pub pr_client: GitHubPrClient,
    pub repo_root: PathBuf,
}

impl<Q: IssueQueue> Factory<Q> {
    /// Build a factory with the given queue and config.
    ///
    /// `repo_root` must be the root of the git repository the factory
    /// watches (not a subdirectory).
    pub fn new(config: FactoryConfig, queue: Q, repo_root: PathBuf) -> Self {
        let pr_client = GitHubPrClient::new(&config.repo, &config.github_token);
        Self {
            config,
            claim_store: ClaimStore::new(),
            queue,
            pr_client,
            repo_root,
        }
    }

    /// Build with an explicit claim store (e.g. SQLite-backed).
    pub fn with_claim_store<S: ClaimStoreTrait>(self, store: ClaimStore) -> Self {
        Self {
            claim_store: store,
            ..self
        }
    }

    /// Run the factory loop forever (or until the first error).
    ///
    /// Each iteration:
    /// 1. Poll the queue.
    /// 2. For each issue, claim a worktree.
    /// 3. Create a claim-bound worktree.
    /// 4. Spawn a worker that runs the placeholder agent loop.
    /// 5. Open a PR (or draft PR in Suggest mode).
    /// 6. Release the claim.
    /// 7. Sleep `poll_interval_secs`.
    pub async fn run(&mut self) -> Result<()> {
        let agent = format!("factory-{}", uuid::Uuid::new_v4());
        loop {
            info!(agent = %agent, "polling queue");
            let issues = match self.queue.poll().await {
                Ok(i) => i,
                Err(e) => {
                    warn!("queue poll failed: {}", e);
                    sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
                    continue;
                }
            };

            if issues.is_empty() {
                sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
                continue;
            }

            let active = self.claim_store.active().len();
            let capacity = self.config.max_workers.saturating_sub(active);
            if capacity == 0 {
                warn!(
                    "max_workers ({}) reached, skipping poll",
                    self.config.max_workers
                );
                sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
                continue;
            }

            for issue in issues.into_iter().take(capacity) {
                let worker_id = format!("worker-{}", uuid::Uuid::new_v4());
                let claim_id = format!("claim-{}", uuid::Uuid::new_v4());
                let feature_slug = sanitize_slug(&issue.title);
                let wp_id = format!("issue-{}", issue.number);
                let branch = format!("feat/{feature_slug}/{wp_id}");

                info!(
                    worker_id = %worker_id,
                    issue = issue.number,
                    branch = %branch,
                    "claiming worktree"
                );

                let mut worker = Worker::new(&worker_id, &claim_id, issue.id, 3600);
                let maybe_claim = worker.claim_worktree(&mut self.claim_store, &branch, &agent);
                let claim = match maybe_claim {
                    Some(c) => c,
                    None => {
                        warn!(claim_id = %claim_id, "resource already claimed, skipping issue");
                        continue;
                    }
                };

                // Create claim-bound worktree.
                let wt_path = match ClaimBoundWorktree::create(
                    self.repo_root.clone(),
                    &feature_slug,
                    &wp_id,
                    &claim,
                    &mut self.claim_store,
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("worktree creation failed: {}", e);
                        let _ = self.claim_store.release(&claim_id);
                        continue;
                    }
                };

                info!(path = %wt_path.display(), "worktree created");

                // Placeholder agent loop.
                if let Err(e) = worker.run_placeholder().await {
                    warn!(worker_id = %worker_id, "agent loop error: {}", e);
                }

                // PR creation (or draft in Suggest mode).
                let pr = GitHubPr {
                    branch: branch.clone(),
                    title: format!("factory: {}", issue.title),
                    body: format!(
                        "Automated PR for issue #{}\n\nTrail:\n```json\n{}\n```",
                        issue.number,
                        worker.trail.to_json().unwrap_or_default()
                    ),
                    base_branch: "main".into(),
                    draft: self.config.autonomy_level == AutonomyLevel::Suggest,
                };

                match self.config.autonomy_level {
                    AutonomyLevel::Suggest => {
                        info!(branch = %branch, "draft PR would be opened (Suggest mode)");
                    }
                    AutonomyLevel::Execute | AutonomyLevel::Merge => {
                        // In a real deployment this calls the GitHub API.
                        // For the scaffold, we log the intent.
                        info!(branch = %branch, "PR would be opened (Execute/Merge mode)");
                    }
                }

                // Log the PR action in the trail.
                worker.trail.log(
                    Action::OpenPr {
                        branch: branch.clone(),
                        title: pr.title.clone(),
                    },
                    Ok(format!("mode={:?}", self.config.autonomy_level)),
                );

                // Release claim.
                let released = self.claim_store.release(&claim_id);
                info!(claim_id = %claim_id, released, "claim released");
            }

            sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
        }
    }

    /// Run a single iteration (for tests). Returns the number of
    /// issues processed.
    pub async fn run_once(&mut self) -> Result<usize> {
        let agent = format!("factory-{}", uuid::Uuid::new_v4());
        let issues = self.queue.poll().await?;
        let active = self.claim_store.active().len();
        let capacity = self.config.max_workers.saturating_sub(active);
        let mut processed = 0usize;

        for issue in issues.into_iter().take(capacity) {
            let worker_id = format!("worker-{}", uuid::Uuid::new_v4());
            let claim_id = format!("claim-{}", uuid::Uuid::new_v4());
            let feature_slug = sanitize_slug(&issue.title);
            let wp_id = format!("issue-{}", issue.number);
            let branch = format!("feat/{feature_slug}/{wp_id}");

            let mut worker = Worker::new(&worker_id, &claim_id, issue.id, 3600);
            let maybe_claim = worker.claim_worktree(&mut self.claim_store, &branch, &agent);
            let claim = match maybe_claim {
                Some(c) => c,
                None => continue,
            };

            let _wt_path = ClaimBoundWorktree::create(
                self.repo_root.clone(),
                &feature_slug,
                &wp_id,
                &claim,
                &mut self.claim_store,
            )?;

            worker.run_placeholder().await?;

            worker.trail.log(
                Action::OpenPr {
                    branch: branch.clone(),
                    title: issue.title.clone(),
                },
                Ok(format!("mode={:?}", self.config.autonomy_level)),
            );

            self.claim_store.release(&claim_id);
            processed += 1;
        }

        Ok(processed)
    }
}

/// Sanitize an issue title into a filesystem-safe slug.
fn sanitize_slug(title: &str) -> String {
    title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}
