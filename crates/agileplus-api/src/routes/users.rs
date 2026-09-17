//! User route handlers.
//!
//! - GET  /api/v1/users         → list all users
//! - POST /api/v1/users         → create user
//! - GET  /api/v1/users/{id}     → get user by id
//!
//! Traceability: WP12-T080 (domain-wiring)

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use agileplus_domain::domain::user::{User, UserRole};
use agileplus_domain::ports::{ObservabilityPort, StoragePort};
use agileplus_domain::ports::vcs::VcsPort;

use crate::error::ApiError;
use crate::responses::UserResponse;
use crate::state::AppState;

pub fn routes<S, V, O>() -> Router<AppState<S, V, O>>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    Router::new()
        .route("/", get(list_users::<S, V, O>))
        .route("/", post(create_user::<S, V, O>))
        .route("/{id}", get(get_user::<S, V, O>))
}

/// `GET /api/v1/users`
pub async fn list_users<S, V, O>(
    State(app): State<AppState<S, V, O>>,
) -> Result<Json<Vec<UserResponse>>, ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let users = app.storage.list_all_users().await.map_err(ApiError::from)?;
    Ok(Json(users.into_iter().map(UserResponse::from).collect()))
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub display_name: String,
    pub email: String,
    /// "admin" | "member" | "viewer" — defaults to "member"
    pub role: Option<String>,
}

/// `POST /api/v1/users`
pub async fn create_user<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Json(body): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let role: UserRole = body
        .role
        .as_deref()
        .unwrap_or("member")
        .parse()
        .map_err(|e: agileplus_domain::error::DomainError| ApiError::BadRequest(e.to_string()))?;

    let user = User::new(&body.display_name, &body.email, role)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let id = app.storage.create_user(&user).await.map_err(ApiError::from)?;
    let created = User { id, ..user };
    Ok((StatusCode::CREATED, Json(UserResponse::from(created))))
}

/// `GET /api/v1/users/{id}`
pub async fn get_user<S, V, O>(
    State(app): State<AppState<S, V, O>>,
    Path(id): Path<i64>,
) -> Result<Json<UserResponse>, ApiError>
where
    S: StoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let user = app
        .storage
        .get_user(id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound(format!("User {id} not found")))?;
    Ok(Json(UserResponse::from(user)))
}
