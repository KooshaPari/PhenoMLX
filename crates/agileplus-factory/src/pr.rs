// SPDX-License-Identifier: MIT OR Apache-2.0
//! GitHub PR creation via raw reqwest (octocrab not in workspace deps).

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Parameters for opening a pull request.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubPr {
    pub branch: String,
    pub title: String,
    pub body: String,
    pub base_branch: String,
    pub draft: bool,
}

/// Low-level GitHub PR client.
#[derive(Debug, Clone)]
pub struct GitHubPrClient {
    owner: String,
    repo: String,
    token: String,
    client: reqwest::Client,
}

impl GitHubPrClient {
    pub fn new(repo: impl Into<String>, token: impl Into<String>) -> Self {
        let repo = repo.into();
        let (owner, repo_name) = repo.split_once('/').unwrap_or((&repo, ""));
        Self {
            owner: owner.to_string(),
            repo: repo_name.to_string(),
            token: token.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Open a PR on GitHub.
    pub async fn open(&self, pr: &GitHubPr) -> Result<u64> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls",
            self.owner, self.repo
        );
        let body = serde_json::json!({
            "title": pr.title,
            "body": pr.body,
            "head": pr.branch,
            "base": pr.base_branch,
            "draft": pr.draft,
        });
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "agileplus-factory/0.1.0")
            .header("Accept", "application/vnd.github+json")
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("GitHub PR API returned error: {status} — {text}");
        }
        let json: serde_json::Value = resp.json().await?;
        let number = json
            .get("number")
            .and_then(|n| n.as_u64())
            .ok_or_else(|| anyhow::anyhow!("GitHub PR response missing number"))?;
        Ok(number)
    }
}
