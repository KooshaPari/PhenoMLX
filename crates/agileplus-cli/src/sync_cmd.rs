// SPDX-License-Identifier: MIT OR Apache-2.0
//! `agileplus sync <owner/repo>` — drive the GitHub sync service.
//!
//! Traceability: WP19-T113 (thin CLI adapter over `agileplus_github::sync`)
//!
//! # Design
//!
//! This module is intentionally thin: it owns only argument parsing and
//! output formatting.  All sync logic lives in `agileplus_github::sync`.
//! A `GhDataSource` trait-object is injected so the command can be unit-
//! tested without touching the network.

use anyhow::{bail, Result};
use clap::Args;

use agileplus_domain::ports::story::StoryRepository;
use agileplus_github::sync::{sync_repository, GhDataSource, SyncReport};

/// Arguments for the `sync` subcommand.
#[derive(Debug, Args)]
pub struct SyncArgs {
    /// GitHub repository in `owner/repo` format (e.g. `KooshaPari/AgilePlus`).
    pub repo: String,

    /// Domain project ID to associate synced stories with.
    #[arg(long)]
    pub project: i64,

    /// Domain epic ID to associate synced stories with.
    #[arg(long)]
    pub epic: i64,

    /// GitHub personal access token.  Falls back to `GITHUB_TOKEN` env var.
    #[arg(long, env = "GITHUB_TOKEN")]
    pub token: Option<String>,

    /// GitHub API base URL (override for testing / GHES).
    #[arg(long, default_value = "https://api.github.com", hide = true)]
    pub api_base: String,
}

/// Parse `"owner/repo"` → `(owner, repo)`.
fn split_repo(repo: &str) -> Result<(&str, &str)> {
    match repo.split_once('/') {
        Some((owner, name)) if !owner.is_empty() && !name.is_empty() => Ok((owner, name)),
        _ => bail!("invalid repo format '{repo}': expected owner/repo (e.g. KooshaPari/AgilePlus)"),
    }
}

/// Print a human-readable summary of a [`SyncReport`].
pub fn print_report(report: &SyncReport) {
    let mapped = report.stories.len();
    let skipped = report.skipped.len();

    println!("Sync complete.");
    println!("  Stories mapped : {mapped}");
    println!("  Items skipped  : {skipped}");

    if !report.skipped.is_empty() {
        println!();
        println!("Skipped items:");
        for (number, reason) in &report.skipped {
            println!("  #{number:>5}  {reason}");
        }
    }
}

/// Entry point called by `main.rs`.
/// Entry point called by `main.rs`.
///
/// Accepts an optional `source` override for dependency injection in tests.
/// When `None`, a live `LiveGhDataSource` is constructed from `args`.
///
/// If `story_repo` is provided, synced stories are persisted via
/// `upsert_by_requirement_id` (idempotent re-sync).
pub async fn run(
    args: SyncArgs,
    source: Option<Box<dyn GhDataSource>>,
    story_repo: Option<&dyn StoryRepository>,
) -> Result<()> {
    let (owner, repo_name) = split_repo(&args.repo)?;

    let report = if let Some(src) = source {
        // test / injection path
        sync_repository(src.as_ref(), args.project, args.epic).await?
    } else {
        // production path: require a token
        let token = args.token.as_deref().unwrap_or("").to_string();
        if token.is_empty() {
            bail!("a GitHub token is required: use --token or GITHUB_TOKEN env var");
        }
        let live =
            agileplus_github::sync::LiveGhDataSource::new(&args.api_base, token, owner, repo_name);
        sync_repository(&live, args.project, args.epic).await?
    };

    // Persist synced stories when a repository port is available.
    if let Some(repo) = story_repo {
        let mut persisted = 0i32;
        let mut persist_errors: Vec<String> = Vec::new();
        for story in &report.stories {
            match repo.upsert_by_requirement_id(story).await {
                Ok(_id) => persisted += 1,
                Err(e) => persist_errors.push(format!(
                    "{}: {e}",
                    story.requirement_id.as_deref().unwrap_or("<no req_id>")
                )),
            }
        }
        println!("  Persisted     : {persisted}");
        if !persist_errors.is_empty() {
            println!("  Persist errors: {}", persist_errors.len());
            for err in &persist_errors {
                println!("    {err}");
            }
        }
    }

    print_report(&report);
    Ok(())
}
// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use agileplus_github::map::{GhIssue, GhPullRequest};
    use async_trait::async_trait;

    // ── Fake data source ──────────────────────────────────────────────────────

    struct FakeSource {
        issues: Vec<GhIssue>,
        prs: Vec<GhPullRequest>,
    }

    impl FakeSource {
        fn new(issues: Vec<GhIssue>, prs: Vec<GhPullRequest>) -> Self {
            Self { issues, prs }
        }
    }

    #[async_trait]
    impl GhDataSource for FakeSource {
        async fn list_issues(&self) -> Result<Vec<GhIssue>, anyhow::Error> {
            Ok(self.issues.clone())
        }
        async fn list_prs(&self) -> Result<Vec<GhPullRequest>, anyhow::Error> {
            Ok(self.prs.clone())
        }
    }

    fn issue(n: i64, title: &str) -> GhIssue {
        GhIssue {
            number: n,
            title: title.to_string(),
            body: None,
            state: "open".to_string(),
            user_login: None,
            user_email: None,
            user_avatar_url: None,
        }
    }

    fn pr(n: i64, title: &str) -> GhPullRequest {
        GhPullRequest {
            number: n,
            title: title.to_string(),
            body: None,
            state: "open".to_string(),
            merged: false,
            user_login: None,
            user_email: None,
            user_avatar_url: None,
        }
    }

    // ── split_repo ────────────────────────────────────────────────────────────

    #[test]
    fn split_repo_ok() {
        let (owner, name) = split_repo("KooshaPari/AgilePlus").unwrap();
        assert_eq!(owner, "KooshaPari");
        assert_eq!(name, "AgilePlus");
    }

    #[test]
    fn split_repo_no_slash_is_error() {
        assert!(split_repo("nodash").is_err());
    }

    #[test]
    fn split_repo_empty_owner_is_error() {
        assert!(split_repo("/repo").is_err());
    }

    #[test]
    fn split_repo_empty_name_is_error() {
        assert!(split_repo("owner/").is_err());
    }

    // ── run (integration-style with stub source) ──────────────────────────────

    fn make_args(repo: &str) -> SyncArgs {
        SyncArgs {
            repo: repo.to_string(),
            project: 1,
            epic: 2,
            token: Some("tok".to_string()),
            api_base: "https://api.github.com".to_string(),
        }
    }

    #[tokio::test]
    async fn run_with_stub_reports_mapped_count() {
        let src = Box::new(FakeSource::new(
            vec![issue(1, "Issue A"), issue(2, "Issue B")],
            vec![pr(10, "PR C")],
        ));
        // Should succeed: 3 stories, 0 skipped
        let result = run(make_args("owner/repo"), Some(src), None).await;
        assert!(result.is_ok(), "expected Ok, got {result:?}");
    }

    #[tokio::test]
    async fn run_skips_bad_items() {
        let bad = GhIssue {
            number: 99,
            title: "   ".to_string(), // empty → skipped
            body: None,
            state: "open".to_string(),
            user_login: None,
            user_email: None,
            user_avatar_url: None,
        };
        let src = Box::new(FakeSource::new(vec![bad, issue(1, "Good issue")], vec![]));
        let result = run(make_args("owner/repo"), Some(src), None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn run_invalid_repo_format_returns_error() {
        let src = Box::new(FakeSource::new(vec![], vec![]));
        let args = make_args("nodash");
        // inject source so we still exercise the early bail
        let result = run(args, Some(src), None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("owner/repo"));
    }

    // ── print_report (output formatting) ─────────────────────────────────────

    #[test]
    fn print_report_no_skipped() {
        // Smoke-test: should not panic.
        let report = SyncReport {
            stories: vec![],
            skipped: vec![],
        };
        print_report(&report); // prints to stdout; asserted not to panic
    }

    #[test]
    fn print_report_with_skipped() {
        let report = SyncReport {
            stories: vec![],
            skipped: vec![
                (42, "empty title".to_string()),
                (99, "unknown state".to_string()),
            ],
        };
        print_report(&report);
    }
}
