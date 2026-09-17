//! Octocrab-based GitHub read client.
//!
//! Provides `list_issues` and `list_prs` using the octocrab SDK.
//! Types are octocrab's own — domain mapping is deferred to a later phase.
//!
//! Traceability: feat/agileplus-github-impl

use thiserror::Error;

/// Errors returned by [`GitHubClient`].
#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to build octocrab client: {0}")]
    Build(#[from] octocrab::Error),

    #[error("GitHub API request failed: {0}")]
    Api(String),
}

/// GitHub read client backed by octocrab.
pub struct GitHubClient {
    client: octocrab::Octocrab,
}

impl GitHubClient {
    /// Construct a new client authenticated with the given personal access token.
    pub fn new(token: &str) -> Result<Self, Error> {
        let client = octocrab::OctocrabBuilder::new()
            .personal_token(token.to_string())
            .build()?;
        Ok(Self { client })
    }

    /// List open issues for the given `owner/repo`.
    ///
    /// Returns octocrab's [`octocrab::models::issues::Issue`] directly;
    /// domain mapping is left to callers.
    pub async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<octocrab::models::issues::Issue>, Error> {
        let page = self
            .client
            .issues(owner, repo)
            .list()
            .state(octocrab::params::State::Open)
            .per_page(100)
            .send()
            .await
            .map_err(|e| Error::Api(e.to_string()))?;

        Ok(page.items)
    }

    /// List open pull requests for the given `owner/repo`.
    ///
    /// Returns octocrab's [`octocrab::models::pulls::PullRequest`] directly.
    pub async fn list_prs(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<octocrab::models::pulls::PullRequest>, Error> {
        let page = self
            .client
            .pulls(owner, repo)
            .list()
            .state(octocrab::params::State::Open)
            .per_page(100)
            .send()
            .await
            .map_err(|e| Error::Api(e.to_string()))?;

        Ok(page.items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_constructs() {
        // Verifies the builder accepts any non-empty token string without
        // making a network call.
        let result = GitHubClient::new("dummy-token-for-test");
        assert!(result.is_ok(), "GitHubClient::new should succeed with a dummy token");
    }
}
