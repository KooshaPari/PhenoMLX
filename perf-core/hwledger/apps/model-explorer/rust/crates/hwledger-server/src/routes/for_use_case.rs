//! `POST /v1/for-use-case` — filter by use-case facet.
//!
//! Body: `{ use_case: "agentic"|"coding"|"reasoning"|"embedding",
//!          text?: string, limit?: number }`.

use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
use serde::Deserialize;

use crate::service::{self, ServiceError, UseCase};
use crate::AppState;

/// Mount the `for-use-case` routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/for-use-case", post(for_use_case))
}

/// Body for `POST /v1/for-use-case`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ForUseCaseRequest {
    /// Which use case to score against.
    pub use_case: UseCase,
    /// Optional free-text query to combine with the use-case filter.
    #[serde(default)]
    pub text: Option<String>,
    /// Cap on returned rows.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// `POST /v1/for-use-case` — use-case filtered search.
async fn for_use_case(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ForUseCaseRequest>,
) -> Result<impl IntoResponse, ServiceError> {
    let limit = req.limit.unwrap_or(10);
    let results = service::for_use_case(&state.index, req.use_case, req.text.as_deref(), limit).await?;
    Ok(Json(serde_json::json!({
        "use_case": req.use_case.as_str(),
        "text": req.text,
        "limit": limit,
        "results": results,
    })))
}