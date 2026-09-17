//! Epic route handlers.
//!
//! - GET  /api/v1/epics/{id}              → get epic
//! - POST /api/v1/epics                  → create epic
//! - POST /api/v1/epics/{id}/transition   → transition epic status
//! - GET  /api/v1/epics/{id}/stories      → list stories for epic
//!
//! Traceability: WP12-T080 (domain-wiring)

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use agileplus_domain::domain::epic::{Epic, EpicStatus};
use agileplus_domain::ports::{ObservabilityPort, StoragePort};
use agileplus_domain::ports::vcs::VcsPort;

use crate::error::ApiError;
use crate::responses::{EpicResponse, StoryResponse};
use crate::state::AppState;

pub fn routes<S, V, O>() -> Router<AppState<S, V, O>>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    Router::new()
        .route("/", post(create_epic::<S, V, O>))
        .route("/{id}", get(get_epic::<S, V, O>))
        .route("/{id}/transition", post(transition_epic::<S, V, O>))
        .route("/{id}/stories", get(list_stories_for_epic::<S, V, O>))
}

#[derive(Debug, Deserialize)]
pub struct CreateEpicRequest {
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
}

/// `POST /api/v1/epics`
pub async fn create_epic<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Json(body): Json<CreateEpicRequest>,
) -> Result<(StatusCode, Json<EpicResponse>), ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let mut epic = Epic::new(body.project_id, &body.title)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    epic.description = body.description;

    let id = app.storage.create_epic(&epic).await.map_err(ApiError::from)?;
    let created = Epic { id, ..epic };
    Ok((StatusCode::CREATED, Json(EpicResponse::from(created))))
}

/// `GET /api/v1/epics/{id}`
pub async fn get_epic<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Path(id): Path<i64>,
) -> Result<Json<EpicResponse>, ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let epic = app
        .storage
        .get_epic(id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound(format!("Epic {id} not found")))?;
    Ok(Json(EpicResponse::from(epic)))
}

#[derive(Debug, Deserialize)]
pub struct TransitionEpicRequest {
    pub target_status: String,
}

/// `POST /api/v1/epics/{id}/transition`
pub async fn transition_epic<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Path(id): Path<i64>,
    Json(body): Json<TransitionEpicRequest>,
) -> Result<Json<EpicResponse>, ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let mut epic = app
        .storage
        .get_epic(id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound(format!("Epic {id} not found")))?;

    let target: EpicStatus = body
        .target_status
        .parse()
        .map_err(|e: agileplus_domain::error::DomainError| ApiError::BadRequest(e.to_string()))?;
    epic.transition_status(target).map_err(ApiError::from)?;

    app.storage
        .update_epic_status(id, epic.status)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(EpicResponse::from(epic)))
}

/// `GET /api/v1/epics/{id}/stories`
pub async fn list_stories_for_epic<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<StoryResponse>>, ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    // verify epic exists
    let _ = app
        .storage
        .get_epic(id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound(format!("Epic {id} not found")))?;

    let stories = app.storage.list_stories_by_epic(id).await.map_err(ApiError::from)?;
    Ok(Json(stories.into_iter().map(StoryResponse::from).collect()))
}
