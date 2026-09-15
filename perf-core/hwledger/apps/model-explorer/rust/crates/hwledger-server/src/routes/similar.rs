//! `GET /v1/models/{author}/{name}/similar?limit=N` — "more like this" lookup.
//!
//! v1 simply re-issues a BM25 query using the model's id tokens; the
//! seed itself is filtered out of the returned rows.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::service::{self, ServiceError};
use crate::AppState;

/// Mount the `similar` routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/models/:author/:name/similar", get(similar))
}

/// Query-string parameters for the `similar` endpoint.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SimilarParams {
    /// Cap on returned rows. Defaults to `10`.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// `GET /v1/models/:author/:name/similar` — more like this.
async fn similar(
    State(state): State<Arc<AppState>>,
    Path((author, name)): Path<(String, String)>,
    Query(params): Query<SimilarParams>,
) -> Result<impl IntoResponse, ServiceError> {
    let id = format!("{author}/{name}");
    let limit = params.limit.unwrap_or(10);
    let results = service::similar_to(&state.index, &id, limit).await?;
    Ok(Json(serde_json::json!({
        "seed": id,
        "limit": limit,
        "results": results,
    })))
}