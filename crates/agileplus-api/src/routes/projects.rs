//! Project route handlers.
//!
//! - GET  /api/v1/projects              → list all projects
//! - POST /api/v1/projects              → create project
//! - GET  /api/v1/projects/{slug}        → get project by slug
//! - GET  /api/v1/projects/{slug}/epics  → list epics for project
//!
//! Traceability: WP12-T080 (domain-wiring)

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use agileplus_domain::domain::project::Project;
use agileplus_domain::ports::{ObservabilityPort, StoragePort};
use agileplus_domain::ports::vcs::VcsPort;

use crate::error::ApiError;
use crate::responses::{EpicResponse, ProjectResponse};
use crate::state::AppState;

pub fn routes<S, V, O>() -> Router<AppState<S, V, O>>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    Router::new()
        .route("/", get(list_projects::<S, V, O>))
        .route("/", post(create_project::<S, V, O>))
        .route("/{slug}", get(get_project::<S, V, O>))
        .route("/{slug}/epics", get(list_epics_for_project::<S, V, O>))
}

/// `GET /api/v1/projects`
pub async fn list_projects<S, V, O>(
    State(app): State<AppState<S, V, O>>,
) -> Result<Json<Vec<ProjectResponse>>, ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let projects = app.storage.list_all_projects().await.map_err(ApiError::from)?;
    Ok(Json(projects.into_iter().map(ProjectResponse::from).collect()))
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
}

/// `POST /api/v1/projects`
pub async fn create_project<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Json(body): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let slug = body.slug.unwrap_or_else(|| Project::slug_from_name(&body.name));
    let mut project = Project::new(&body.name, &slug).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    project.description = body.description;

    let id = app.storage.create_project(&project).await.map_err(ApiError::from)?;
    let created = Project { id, ..project };
    Ok((StatusCode::CREATED, Json(ProjectResponse::from(created))))
}

/// `GET /api/v1/projects/{slug}`
pub async fn get_project<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Path(slug): Path<String>,
) -> Result<Json<ProjectResponse>, ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let project = app
        .storage
        .get_project_by_slug(&slug)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound(format!("Project '{slug}' not found")))?;
    Ok(Json(ProjectResponse::from(project)))
}

/// `GET /api/v1/projects/{slug}/epics`
pub async fn list_epics_for_project<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Path(slug): Path<String>,
) -> Result<Json<Vec<EpicResponse>>, ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let project = app
        .storage
        .get_project_by_slug(&slug)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound(format!("Project '{slug}' not found")))?;

    let epics = app
        .storage
        .list_epics_by_project(project.id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(epics.into_iter().map(EpicResponse::from).collect()))
}
