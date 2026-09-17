// SPDX-License-Identifier: MIT OR Apache-2.0
//! Issue queue abstraction + GitHub implementation.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A single issue surfaced by the queue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Issue {
    pub id: u64,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub labels: Vec<String>,
    pub state: String,
}

/// Queue abstraction — anything that can yield a list of open issues.
#[async_trait]
pub trait IssueQueue: Send + Sync {
    /// Poll for open issues matching the factory label.
    async fn poll(&self) -> Result<Vec<Issue>>;
}

// ── GitHub implementation ───────────────────────────────────────────────────

/// Fetches issues labeled `factory` from the GitHub REST API v3.
#[derive(Debug, Clone)]
pub struct GitHubIssueQueue {
    owner: String,
    repo: String,
    label: String,
    token: String,
    client: reqwest::Client,
}

impl GitHubIssueQueue {
    pub fn new(
        repo: impl Into<String>,
        label: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        let repo = repo.into();
        let (owner, repo_name) = repo.split_once('/').unwrap_or((&repo, ""));
        Self {
            owner: owner.to_string(),
            repo: repo_name.to_string(),
            label: label.into(),
            token: token.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl IssueQueue for GitHubIssueQueue {
    async fn poll(&self) -> Result<Vec<Issue>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues?labels={}&state=open",
            self.owner, self.repo, self.label
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "agileplus-factory/0.1.0")
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!(
                "GitHub API returned {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }
        let raw: Vec<serde_json::Value> = resp.json().await?;
        let issues: Vec<Issue> = raw
            .into_iter()
            .filter_map(|v| {
                let id = v.get("id")?.as_u64()?;
                let number = v.get("number")?.as_i64()?;
                let title = v.get("title")?.as_str()?.to_string();
                let body = v.get("body").and_then(|b| b.as_str()).map(String::from);
                let labels = v
                    .get("labels")?
                    .as_array()?
                    .iter()
                    .filter_map(|l| l.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect();
                let state = v.get("state")?.as_str()?.to_string();
                Some(Issue {
                    id,
                    number,
                    title,
                    body,
                    labels,
                    state,
                })
            })
            .collect();
        Ok(issues)
    }
}

/// In-memory fake queue for tests.
#[derive(Debug, Default, Clone)]
pub struct FakeIssueQueue {
    issues: std::sync::Arc<tokio::sync::Mutex<Vec<Issue>>>,
}

impl FakeIssueQueue {
    pub fn new(issues: Vec<Issue>) -> Self {
        Self {
            issues: std::sync::Arc::new(tokio::sync::Mutex::new(issues)),
        }
    }

    pub async fn push(&self, issue: Issue) {
        self.issues.lock().await.push(issue);
    }
}

#[async_trait]
impl IssueQueue for FakeIssueQueue {
    async fn poll(&self) -> Result<Vec<Issue>> {
        let mut guard = self.issues.lock().await;
        let out = guard.clone();
        guard.clear();
        Ok(out)
    }
}
