// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mapping from GitHub REST API types → agileplus-domain entities.
//!
//! Traceability: WP19-T115

use agileplus_domain::domain::story::{Story, StoryStatus};
use agileplus_domain::domain::user::{User, UserRole};
use agileplus_domain::error::DomainError;

/// A minimal GitHub issue representation suitable for mapping.
/// Mirrors the fields returned by the GitHub REST API and our own
/// `GitHubIssueResponse`, but kept as a plain struct so tests can
/// construct it without hitting the network.
#[derive(Debug, Clone)]
pub struct GhIssue {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    /// "open" or "closed"
    pub state: String,
    pub user_login: Option<String>,
    pub user_email: Option<String>,
    pub user_avatar_url: Option<String>,
}

/// A minimal GitHub pull-request representation suitable for mapping.
#[derive(Debug, Clone)]
pub struct GhPullRequest {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    /// "open" or "closed" or "merged"
    pub state: String,
    pub merged: bool,
    pub user_login: Option<String>,
    pub user_email: Option<String>,
    pub user_avatar_url: Option<String>,
}

/// Map a GitHub issue state string to a domain `StoryStatus`.
///
/// * `"open"`   → `Todo`
/// * `"closed"` → `Done`
/// * anything else → `Err(DomainError::Validation(…))`
pub fn gh_state_to_story_status(state: &str) -> Result<StoryStatus, DomainError> {
    match state.to_ascii_lowercase().as_str() {
        "open" => Ok(StoryStatus::Todo),
        "closed" => Ok(StoryStatus::Done),
        other => Err(DomainError::Validation(format!(
            "unknown GitHub issue state: '{other}'"
        ))),
    }
}

/// Map a GitHub PR state to a domain `StoryStatus`.
///
/// * `"open"`   → `InProgress`
/// * `"merged"` / `"closed" + merged=true`  → `Done`
/// * `"closed"` (not merged) → `Cancelled`
pub fn gh_pr_state_to_story_status(state: &str, merged: bool) -> Result<StoryStatus, DomainError> {
    match state.to_ascii_lowercase().as_str() {
        "open" => Ok(StoryStatus::InProgress),
        "closed" | "merged" => {
            if merged {
                Ok(StoryStatus::Done)
            } else {
                Ok(StoryStatus::Cancelled)
            }
        }
        other => Err(DomainError::Validation(format!(
            "unknown GitHub PR state: '{other}'"
        ))),
    }
}

/// Convert a GitHub login into a best-effort `User`.
///
/// GitHub doesn't always expose a real email; when absent we synthesise
/// `<login>@github.invalid` so the domain invariant (must contain `@`)
/// is always satisfied.
///
/// The returned `User` has `id = 0` (not yet persisted) and
/// `github_login` set to the login string.
pub fn gh_user_to_domain(
    login: &str,
    email: Option<&str>,
    avatar_url: Option<&str>,
) -> Result<User, DomainError> {
    let effective_email = match email {
        Some(e) if e.contains('@') => e.to_string(),
        _ => format!("{login}@github.invalid"),
    };

    let mut user = User::new(login, &effective_email, UserRole::Member)?;
    user.github_login = Some(login.to_string());
    user.avatar_url = avatar_url.map(str::to_string);
    Ok(user)
}

/// Map a `GhIssue` → domain `Story`.
///
/// * `epic_id` and `project_id` are caller-supplied context (GitHub has no concept of epics).
/// * Title must be non-empty; an empty title returns `Err(DomainError::Validation)`.
/// * Unknown `state` values return `Err(DomainError::Validation)`.
pub fn issue_to_story(
    issue: &GhIssue,
    epic_id: i64,
    project_id: i64,
) -> Result<Story, DomainError> {
    let status = gh_state_to_story_status(&issue.state)?;
    let mut story = Story::new(epic_id, project_id, &issue.title, None)?;
    story.description = issue.body.clone().filter(|b| !b.trim().is_empty());
    story.status = status;
    story.id = issue.number;
    // Stable external key for idempotent upsert (FR-AGP-013).
    story.requirement_id = Some(format!("gh:issue:{}", issue.number));
    Ok(story)
}

/// Map a `GhPullRequest` → domain `Story`.
///
/// PRs are treated as stories: open → InProgress, merged → Done, closed-without-merge → Cancelled.
pub fn pr_to_story(
    pr: &GhPullRequest,
    epic_id: i64,
    project_id: i64,
) -> Result<Story, DomainError> {
    let status = gh_pr_state_to_story_status(&pr.state, pr.merged)?;
    let mut story = Story::new(epic_id, project_id, &pr.title, None)?;
    story.description = pr.body.clone().filter(|b| !b.trim().is_empty());
    story.status = status;
    story.id = pr.number;
    // Stable external key for idempotent upsert (FR-AGP-013).
    story.requirement_id = Some(format!("gh:pr:{}", pr.number));
    Ok(story)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agileplus_domain::domain::story::StoryStatus;
    use agileplus_domain::error::DomainError;

    fn open_issue(title: &str) -> GhIssue {
        GhIssue {
            number: 42,
            title: title.to_string(),
            body: Some("Reproduces on startup.".to_string()),
            state: "open".to_string(),
            user_login: Some("octocat".to_string()),
            user_email: None,
            user_avatar_url: None,
        }
    }

    // ── issue_to_story ────────────────────────────────────────────────────────

    #[test]
    fn open_issue_maps_to_todo_story() {
        let issue = open_issue("Fix login crash");
        let story = issue_to_story(&issue, 1, 10).unwrap();
        assert_eq!(story.title, "Fix login crash");
        assert_eq!(story.status, StoryStatus::Todo);
        assert_eq!(story.id, 42);
        assert_eq!(story.epic_id, 1);
        assert_eq!(story.project_id, 10);
        assert_eq!(story.description.as_deref(), Some("Reproduces on startup."));
    }

    #[test]
    fn closed_issue_maps_to_done_story() {
        let mut issue = open_issue("Deploy hotfix");
        issue.state = "closed".to_string();
        let story = issue_to_story(&issue, 2, 20).unwrap();
        assert_eq!(story.status, StoryStatus::Done);
    }

    #[test]
    fn empty_title_returns_validation_error() {
        let issue = open_issue("   ");
        let err = issue_to_story(&issue, 1, 1).unwrap_err();
        assert!(
            matches!(err, DomainError::Validation(_)),
            "expected Validation, got {err:?}"
        );
    }

    #[test]
    fn unknown_state_returns_validation_error() {
        let mut issue = open_issue("Some issue");
        issue.state = "draft".to_string();
        let err = issue_to_story(&issue, 1, 1).unwrap_err();
        assert!(
            matches!(err, DomainError::Validation(_)),
            "expected Validation, got {err:?}"
        );
    }

    // ── pr_to_story ───────────────────────────────────────────────────────────

    #[test]
    fn open_pr_maps_to_in_progress() {
        let pr = GhPullRequest {
            number: 99,
            title: "feat: add dark mode".to_string(),
            body: Some("Implements dark mode toggle.".to_string()),
            state: "open".to_string(),
            merged: false,
            user_login: Some("dev".to_string()),
            user_email: None,
            user_avatar_url: None,
        };
        let story = pr_to_story(&pr, 3, 30).unwrap();
        assert_eq!(story.status, StoryStatus::InProgress);
        assert_eq!(story.id, 99);
    }

    #[test]
    fn merged_pr_maps_to_done() {
        let pr = GhPullRequest {
            number: 100,
            title: "fix: null deref".to_string(),
            body: None,
            state: "closed".to_string(),
            merged: true,
            user_login: None,
            user_email: None,
            user_avatar_url: None,
        };
        let story = pr_to_story(&pr, 3, 30).unwrap();
        assert_eq!(story.status, StoryStatus::Done);
        assert!(story.description.is_none());
    }

    #[test]
    fn closed_unmerged_pr_maps_to_cancelled() {
        let pr = GhPullRequest {
            number: 101,
            title: "wip: experiment".to_string(),
            body: None,
            state: "closed".to_string(),
            merged: false,
            user_login: None,
            user_email: None,
            user_avatar_url: None,
        };
        let story = pr_to_story(&pr, 3, 30).unwrap();
        assert_eq!(story.status, StoryStatus::Cancelled);
    }

    // ── gh_user_to_domain ─────────────────────────────────────────────────────

    #[test]
    fn user_without_email_gets_synthetic_email() {
        let user = gh_user_to_domain("torvalds", None, None).unwrap();
        assert_eq!(user.email, "torvalds@github.invalid");
        assert_eq!(user.github_login.as_deref(), Some("torvalds"));
    }

    #[test]
    fn user_with_real_email_uses_it() {
        let user =
            gh_user_to_domain("alice", Some("alice@example.com"), Some("https://avatar")).unwrap();
        assert_eq!(user.email, "alice@example.com");
        assert_eq!(user.avatar_url.as_deref(), Some("https://avatar"));
    }
}
