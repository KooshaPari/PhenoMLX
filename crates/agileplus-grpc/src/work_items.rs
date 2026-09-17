// SPDX-License-Identifier: MIT OR Apache-2.0
//! WorkItemsService — gRPC adapter for Projects, Epics, Stories, and
//! GitHub repository sync.
//!
//! Hexagonal: this adapter delegates all reads to the `StoragePort` and all
//! GitHub sync operations to `agileplus_github::sync::sync_repository` via an
//! in-process `GhDataSource` implementation.  No business logic lives here.
//!
//! Traceability: FR-AGP-011

use tonic::{Request, Response, Status};

use agileplus_domain::ports::{AgentPort, ObservabilityPort, ReviewPort, StoragePort, VcsPort};
use agileplus_proto::agileplus::v1::{
    work_items_service_server::WorkItemsService, EpicProto, ListEpicsRequest, ListEpicsResponse,
    ListProjectsRequest, ListProjectsResponse, ListStoriesRequest, ListStoriesResponse,
    ProjectProto, StoryProto, SyncRepositoryRequest, SyncRepositoryResponse,
};

use crate::server::{domain_error_to_status, AgilePlusCoreServer};

// ── WorkItemsService impl ─────────────────────────────────────────────────────

#[tonic::async_trait]
impl<S, V, A, R, O> WorkItemsService for AgilePlusCoreServer<S, V, A, R, O>
where
    S: StoragePort + 'static,
    V: VcsPort + 'static,
    A: AgentPort + 'static,
    R: ReviewPort + 'static,
    O: ObservabilityPort + 'static,
{
    /// List all projects.
    async fn list_projects(
        &self,
        _request: Request<ListProjectsRequest>,
    ) -> Result<Response<ListProjectsResponse>, Status> {
        let projects = self
            .storage
            .list_all_projects()
            .await
            .map_err(domain_error_to_status)?;

        Ok(Response::new(ListProjectsResponse {
            projects: projects.into_iter().map(project_to_proto).collect(),
        }))
    }

    /// List epics belonging to a project.
    async fn list_epics(
        &self,
        request: Request<ListEpicsRequest>,
    ) -> Result<Response<ListEpicsResponse>, Status> {
        let project_id = request.into_inner().project_id;
        let epics = self
            .storage
            .list_epics_by_project(project_id)
            .await
            .map_err(domain_error_to_status)?;

        Ok(Response::new(ListEpicsResponse {
            epics: epics.into_iter().map(epic_to_proto).collect(),
        }))
    }

    /// List stories belonging to an epic.
    async fn list_stories(
        &self,
        request: Request<ListStoriesRequest>,
    ) -> Result<Response<ListStoriesResponse>, Status> {
        let epic_id = request.into_inner().epic_id;
        let stories = self
            .storage
            .list_stories_by_epic(epic_id)
            .await
            .map_err(domain_error_to_status)?;

        Ok(Response::new(ListStoriesResponse {
            stories: stories.into_iter().map(story_to_proto).collect(),
        }))
    }

    /// Trigger a GitHub repository sync and persist mapped stories.
    ///
    /// Requires `GITHUB_TOKEN` environment variable to be set.  Returns an
    /// error when the token is absent rather than silently succeeding with
    /// zero stories — callers should configure the token or handle the error.
    async fn sync_repository(
        &self,
        request: Request<SyncRepositoryRequest>,
    ) -> Result<Response<SyncRepositoryResponse>, Status> {
        let req = request.into_inner();

        // Parse "owner/repo" from the request.
        let parts: Vec<&str> = req.repo.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(Status::invalid_argument(
                "repo must be in 'owner/repo' format",
            ));
        }
        let (owner, repo_name) = (parts[0].to_string(), parts[1].to_string());

        // Require a GitHub token.
        let token = std::env::var("GITHUB_TOKEN").map_err(|_| {
            Status::unauthenticated("GITHUB_TOKEN env var is required for SyncRepository")
        })?;

        // Build the live GitHub data source.
        let source = agileplus_github::sync::LiveGhDataSource::new(
            "https://api.github.com",
            token,
            owner,
            repo_name,
        );

        // Run the sync.
        let report = agileplus_github::sync::sync_repository(&source, req.project_id, req.epic_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let stories_synced = report.stories.len() as i32;
        let stories_skipped = report.skipped.len() as i32;
        let errors: Vec<String> = report
            .skipped
            .iter()
            .map(|(n, reason)| format!("#{n}: {reason}"))
            .collect();

        // Persist synced stories via the storage port.
        for story in &report.stories {
            self.storage
                .upsert_story_by_requirement_id(story)
                .await
                .map_err(domain_error_to_status)?;
        }

        Ok(Response::new(SyncRepositoryResponse {
            stories_synced,
            stories_skipped,
            errors,
        }))
    }
}

// ── Conversion helpers ────────────────────────────────────────────────────────

fn project_to_proto(p: agileplus_domain::domain::project::Project) -> ProjectProto {
    ProjectProto {
        id: p.id,
        slug: p.slug,
        name: p.name,
        description: p.description.unwrap_or_default(),
        created_at: p.created_at.to_rfc3339(),
        updated_at: p.updated_at.to_rfc3339(),
    }
}

fn epic_to_proto(e: agileplus_domain::domain::epic::Epic) -> EpicProto {
    EpicProto {
        id: e.id,
        project_id: e.project_id,
        title: e.title,
        description: e.description.unwrap_or_default(),
        status: format!("{:?}", e.status).to_lowercase(),
        created_at: e.created_at.to_rfc3339(),
        updated_at: e.updated_at.to_rfc3339(),
    }
}

fn story_to_proto(s: agileplus_domain::domain::story::Story) -> StoryProto {
    StoryProto {
        id: s.id,
        epic_id: s.epic_id,
        project_id: s.project_id,
        title: s.title,
        description: s.description.unwrap_or_default(),
        status: format!("{:?}", s.status).to_lowercase(),
        points: s.points.unwrap_or(0) as i32,
        requirement_id: s.requirement_id.unwrap_or_default(),
        created_at: s.created_at.to_rfc3339(),
        updated_at: s.updated_at.to_rfc3339(),
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use agileplus_domain::domain::epic::Epic;
    use agileplus_domain::domain::project::Project;
    use agileplus_domain::domain::story::Story;

    #[test]
    fn project_conversion_roundtrip() {
        let p = Project::new("My Project", "my-project").unwrap();
        let proto = project_to_proto(p);
        assert_eq!(proto.slug, "my-project");
        assert_eq!(proto.name, "My Project");
        assert!(proto.description.is_empty());
        assert!(!proto.created_at.is_empty());
    }

    #[test]
    fn epic_conversion_roundtrip() {
        let e = Epic::new(1, "Auth Epic").unwrap();
        let proto = epic_to_proto(e);
        assert_eq!(proto.title, "Auth Epic");
        assert_eq!(proto.project_id, 1);
        assert_eq!(proto.status, "backlog");
        assert!(!proto.created_at.is_empty());
    }

    #[test]
    fn story_conversion_roundtrip() {
        let s = Story::new(1, 1, "Login story", Some(3)).unwrap();
        let proto = story_to_proto(s);
        assert_eq!(proto.title, "Login story");
        assert_eq!(proto.epic_id, 1);
        assert_eq!(proto.points, 3);
        assert_eq!(proto.status, "todo");
    }

    #[test]
    fn story_conversion_no_points() {
        let s = Story::new(2, 1, "No-points story", None).unwrap();
        let proto = story_to_proto(s);
        assert_eq!(proto.points, 0);
    }
}
