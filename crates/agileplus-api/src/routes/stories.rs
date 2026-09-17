//! Story route handlers.
//!
//! - GET  /api/v1/stories/{id}            → get story
//! - POST /api/v1/stories                → create story
//! - POST /api/v1/stories/{id}/transition → transition story status
//!
//! Traceability: WP12-T080 (domain-wiring)

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use agileplus_domain::domain::story::{Story, StoryStatus};
use agileplus_domain::ports::{ObservabilityPort, StoragePort};
use agileplus_domain::ports::vcs::VcsPort;

use crate::error::ApiError;
use crate::responses::StoryResponse;
use crate::state::AppState;

pub fn routes<S, V, O>() -> Router<AppState<S, V, O>>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    Router::new()
        .route("/", post(create_story::<S, V, O>))
        .route("/{id}", get(get_story::<S, V, O>))
        .route("/{id}/transition", post(transition_story::<S, V, O>))
}

#[derive(Debug, Deserialize)]
pub struct CreateStoryRequest {
    pub epic_id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub points: Option<u32>,
}

/// `POST /api/v1/stories`
pub async fn create_story<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Json(body): Json<CreateStoryRequest>,
) -> Result<(StatusCode, Json<StoryResponse>), ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let mut story = Story::new(body.epic_id, body.project_id, &body.title, body.points)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    story.description = body.description;

    let id = app.storage.create_story(&story).await.map_err(ApiError::from)?;
    let created = Story { id, ..story };
    Ok((StatusCode::CREATED, Json(StoryResponse::from(created))))
}

/// `GET /api/v1/stories/{id}`
pub async fn get_story<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Path(id): Path<i64>,
) -> Result<Json<StoryResponse>, ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let story = app
        .storage
        .get_story(id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound(format!("Story {id} not found")))?;
    Ok(Json(StoryResponse::from(story)))
}

#[derive(Debug, Deserialize)]
pub struct TransitionStoryRequest {
    pub target_status: String,
}

/// `POST /api/v1/stories/{id}/transition`
pub async fn transition_story<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Path(id): Path<i64>,
    Json(body): Json<TransitionStoryRequest>,
) -> Result<Json<StoryResponse>, ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let mut story = app
        .storage
        .get_story(id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound(format!("Story {id} not found")))?;

    let target: StoryStatus = body
        .target_status
        .parse()
        .map_err(|e: agileplus_domain::error::DomainError| ApiError::BadRequest(e.to_string()))?;
    story.transition_status(target).map_err(ApiError::from)?;

    app.storage
        .update_story_status(id, story.status)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(StoryResponse::from(story)))
}
