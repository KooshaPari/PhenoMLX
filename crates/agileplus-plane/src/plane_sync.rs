// SPDX-License-Identifier: MIT OR Apache-2.0
use std::env;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Serialize;

use agileplus_domain::domain::story::{Story, StoryStatus};
use agileplus_domain::error::DomainError;
use agileplus_domain::ports::{
    plane_state_to_story_status, story_status_to_plane_state, PlaneIssue, PlaneProject,
    PlaneSyncPort,
};

#[derive(Debug, Clone)]
pub struct PlaneClient {
    base_url: String,
    workspace: String,
    token: String,
    http: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct StoryIssueRequest {
    name: String,
    state: Option<String>,
    priority: Option<i32>,
}

impl PlaneClient {
    pub fn new(base_url: String, workspace: String, token: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            workspace,
            token,
            http: reqwest::Client::new(),
        }
    }

    pub fn from_env() -> Result<Self> {
        let base_url = env::var("PLANE_BASE_URL").context("PLANE_BASE_URL is not set")?;
        let workspace = env::var("PLANE_WORKSPACE").context("PLANE_WORKSPACE is not set")?;
        let token = env::var("PLANE_TOKEN").context("PLANE_TOKEN is not set")?;
        Ok(Self::new(base_url, workspace, token))
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    fn projects_url(&self) -> String {
        self.endpoint(&format!("api/v1/workspaces/{}/projects/", self.workspace))
    }

    fn issues_url(&self, project_identifier: &str) -> String {
        self.endpoint(&format!(
            "api/v1/workspaces/{}/projects/{}/work-items/",
            self.workspace, project_identifier
        ))
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: String) -> Result<T> {
        let response = self
            .http
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .await
            .context("sending GET request to Plane")?;
        Self::read_json(response).await
    }

    async fn post_json<T: serde::de::DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        url: String,
        body: &B,
    ) -> Result<T> {
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .context("sending POST request to Plane")?;
        Self::read_json(response).await
    }

    async fn read_json<T: serde::de::DeserializeOwned>(response: reqwest::Response) -> Result<T> {
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Plane API error {status}: {body}");
        }

        response.json::<T>().await.context("decoding Plane JSON")
    }

    fn map_story_points_to_priority(
        points: Option<u32>,
    ) -> std::result::Result<Option<i32>, DomainError> {
        points
            .map(|value| {
                i32::try_from(value).map_err(|_| {
                    DomainError::Validation("story points exceed Plane priority range".to_string())
                })
            })
            .transpose()
    }

    fn map_priority_to_story_points(
        priority: Option<i32>,
    ) -> std::result::Result<Option<u32>, DomainError> {
        priority
            .map(|value| {
                u32::try_from(value).map_err(|_| {
                    DomainError::Validation(
                        "Plane priority cannot be converted to story points".to_string(),
                    )
                })
            })
            .transpose()
    }

    fn map_story_to_issue(
        &self,
        story: &Story,
    ) -> std::result::Result<StoryIssueRequest, DomainError> {
        Ok(StoryIssueRequest {
            name: story.title.clone(),
            state: Some(story_status_to_plane_state(story.status).to_string()),
            priority: Self::map_story_points_to_priority(story.points)?,
        })
    }

    fn map_issue_to_story(
        &self,
        project_id: i64,
        epic_id: i64,
        issue: &PlaneIssue,
    ) -> std::result::Result<Story, DomainError> {
        let mut story = Story::new(
            epic_id,
            project_id,
            &issue.name,
            Self::map_priority_to_story_points(issue.priority)?,
        )?;
        if let Some(sequence_id) = issue.sequence_id {
            story.id = sequence_id;
        }
        story.status = match issue.state.as_deref() {
            Some(state) => plane_state_to_story_status(state)?,
            None => StoryStatus::Todo,
        };
        Ok(story)
    }

    pub async fn list_projects(&self) -> Result<Vec<PlaneProject>> {
        self.get_json(self.projects_url()).await
    }

    /// PATCH an issue in Plane by its UUID, merging in the supplied JSON body.
    /// Returns `Ok(())` if the API returns 2xx. Non-2xx status is logged as a
    /// warning but not returned as an error so callers remain non-blocking.
    pub async fn patch_issue(
        &self,
        project_identifier: &str,
        issue_id: &str,
        body: &serde_json::Value,
    ) -> Result<()> {
        let url = self.endpoint(&format!(
            "api/v1/workspaces/{}/projects/{}/work-items/{}/",
            self.workspace, project_identifier, issue_id,
        ));
        let response = self
            .http
            .patch(url)
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .context("sending PATCH request to Plane")?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            tracing::warn!(issue_id, %status, body=%text, "Plane PATCH returned non-success");
        }
        Ok(())
    }

    pub async fn sync_story_to_plane(
        &self,
        project_identifier: &str,
        story: &Story,
    ) -> Result<PlaneIssue> {
        let body = self
            .map_story_to_issue(story)
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        self.post_json(self.issues_url(project_identifier), &body)
            .await
    }

    pub async fn sync_from_plane(
        &self,
        project_id: i64,
        epic_id: i64,
        issue: &PlaneIssue,
    ) -> Result<Story> {
        self.map_issue_to_story(project_id, epic_id, issue)
            .map_err(|err| anyhow::anyhow!(err.to_string()))
    }
}

#[async_trait]
impl PlaneSyncPort for PlaneClient {
    async fn list_projects(&self) -> Result<Vec<PlaneProject>, DomainError> {
        self.get_json(self.projects_url())
            .await
            .map_err(|err| DomainError::Storage(err.to_string()))
    }

    async fn sync_story_to_plane(
        &self,
        project_identifier: &str,
        story: &Story,
    ) -> Result<PlaneIssue, DomainError> {
        let body = self.map_story_to_issue(story)?;
        self.post_json(self.issues_url(project_identifier), &body)
            .await
            .map_err(|err| DomainError::Storage(err.to_string()))
    }

    async fn sync_from_plane(
        &self,
        project_id: i64,
        epic_id: i64,
        issue: &PlaneIssue,
    ) -> Result<Story, DomainError> {
        self.map_issue_to_story(project_id, epic_id, issue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> PlaneClient {
        PlaneClient::new(server.uri(), "workspace".into(), "token".into())
    }

    fn sample_story() -> Story {
        Story::new(9, 42, "Story title", Some(5)).unwrap()
    }

    #[test]
    fn new_trims_base_url() {
        let client = PlaneClient::new(
            "https://plane.example.com/".into(),
            "ws".into(),
            "tok".into(),
        );
        assert_eq!(client.base_url, "https://plane.example.com");
    }

    #[test]
    fn plane_state_round_trip_helpers_work() {
        assert_eq!(
            story_status_to_plane_state(Story::new(1, 1, "x", None).unwrap().status),
            "todo"
        );
        assert!(matches!(
            plane_state_to_story_status("done"),
            Ok(agileplus_domain::domain::story::StoryStatus::Done)
        ));
    }

    #[test]
    fn rejects_unknown_plane_state() {
        let err = plane_state_to_story_status("mystery").unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn rejects_priority_overflow_from_story_points() {
        let story = Story::new(1, 1, "big", Some(u32::MAX)).unwrap();
        let err = PlaneClient::new("http://localhost".into(), "ws".into(), "tok".into())
            .map_story_to_issue(&story)
            .unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn rejects_negative_plane_priority_on_pull() {
        let err = PlaneClient::new("http://localhost".into(), "ws".into(), "tok".into())
            .map_issue_to_story(
                1,
                2,
                &PlaneIssue::new("1", "x", Some("todo".into()), Some(-1), Some(99)),
            )
            .unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[tokio::test]
    async fn from_env_reads_configuration() {
        let old_base = env::var("PLANE_BASE_URL").ok();
        let old_workspace = env::var("PLANE_WORKSPACE").ok();
        let old_token = env::var("PLANE_TOKEN").ok();

        unsafe {
            env::set_var("PLANE_BASE_URL", "https://plane.example.com/");
            env::set_var("PLANE_WORKSPACE", "team");
            env::set_var("PLANE_TOKEN", "secret");
        }

        let client = PlaneClient::from_env().unwrap();
        assert_eq!(client.base_url, "https://plane.example.com");
        assert_eq!(client.workspace, "team");
        assert_eq!(client.token, "secret");

        match old_base {
            Some(value) => unsafe { env::set_var("PLANE_BASE_URL", value) },
            None => unsafe { env::remove_var("PLANE_BASE_URL") },
        }
        match old_workspace {
            Some(value) => unsafe { env::set_var("PLANE_WORKSPACE", value) },
            None => unsafe { env::remove_var("PLANE_WORKSPACE") },
        }
        match old_token {
            Some(value) => unsafe { env::set_var("PLANE_TOKEN", value) },
            None => unsafe { env::remove_var("PLANE_TOKEN") },
        }
    }

    #[tokio::test]
    async fn list_projects_uses_workspace_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/workspaces/workspace/projects/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![
                PlaneProject {
                    id: "1".into(),
                    name: "Alpha".into(),
                    identifier: "alpha".into(),
                },
                PlaneProject {
                    id: "2".into(),
                    name: "Beta".into(),
                    identifier: "beta".into(),
                },
            ]))
            .mount(&server)
            .await;

        let result = client(&server).list_projects().await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].identifier, "alpha");
    }

    #[tokio::test]
    async fn list_projects_propagates_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/workspaces/workspace/projects/"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;

        let err = client(&server).list_projects().await.unwrap_err();
        assert!(err.to_string().contains("500"));
    }

    #[tokio::test]
    async fn sync_story_to_plane_posts_mapped_payload() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(
                "/api/v1/workspaces/workspace/projects/proj-1/work-items/",
            ))
            .and(body_partial_json(serde_json::json!({
                "name": "Story title",
                "state": "todo",
                "priority": 5
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "issue-1",
                "name": "Story title",
                "state": "todo",
                "priority": 5,
                "sequence_id": 9001
            })))
            .mount(&server)
            .await;

        let issue = client(&server)
            .sync_story_to_plane("proj-1", &sample_story())
            .await
            .unwrap();
        assert_eq!(issue.id, "issue-1");
        assert_eq!(issue.sequence_id, Some(9001));
    }

    #[tokio::test]
    async fn sync_story_to_plane_rejects_points_overflow() {
        let server = MockServer::start().await;
        let mut story = sample_story();
        story.points = Some(u32::MAX);

        let err = client(&server)
            .sync_story_to_plane("proj-1", &story)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("story points"));
    }

    #[tokio::test]
    async fn sync_story_to_plane_propagates_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(
                "/api/v1/workspaces/workspace/projects/proj-1/work-items/",
            ))
            .respond_with(ResponseTemplate::new(500).set_body_string("nope"))
            .mount(&server)
            .await;

        let err = client(&server)
            .sync_story_to_plane("proj-1", &sample_story())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("500"));
    }

    #[tokio::test]
    async fn sync_from_plane_maps_state_and_priority() {
        let story = PlaneClient::new(
            "http://localhost".into(),
            "workspace".into(),
            "token".into(),
        )
        .sync_from_plane(
            42,
            9,
            &PlaneIssue::new(
                "issue-1",
                "Remote story",
                Some("review".into()),
                Some(3),
                Some(77),
            ),
        )
        .await
        .unwrap();
        assert_eq!(story.project_id, 42);
        assert_eq!(story.epic_id, 9);
        assert_eq!(story.id, 77);
        assert_eq!(story.points, Some(3));
        assert_eq!(
            story.status,
            agileplus_domain::domain::story::StoryStatus::Review
        );
    }

    #[tokio::test]
    async fn sync_from_plane_defaults_missing_state_to_todo() {
        let story = PlaneClient::new(
            "http://localhost".into(),
            "workspace".into(),
            "token".into(),
        )
        .sync_from_plane(
            1,
            2,
            &PlaneIssue::new("issue-1", "Remote story", None, Some(1), None),
        )
        .await
        .unwrap();
        assert_eq!(
            story.status,
            agileplus_domain::domain::story::StoryStatus::Todo
        );
        assert_eq!(story.id, 0);
    }

    #[test]
    fn issue_serializes_cleanly() {
        let issue = PlaneIssue::new("1", "Title", Some("todo".into()), Some(2), Some(11));
        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains("\"sequence_id\":11"));
    }
}
