//! `GET /v1/models/{author}/{name}` — single-model detail lookup.
//!
//! Returns `{ id, found, score, kind, quants }`. `author/name` is
//! canonicalized to `hf::author/name` by the service layer when no
//! source prefix is present. Model ids always have the shape
//! `org/name` so a two-segment path capture is sufficient (axum 0.7
//! does not allow catch-all segments mid-route).

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};

use crate::service;
use crate::AppState;

/// Mount the `detail` routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/models/:author/:name", get(detail))
}

/// `GET /v1/models/:author/:name` — single model detail.
async fn detail(
    State(state): State<Arc<AppState>>,
    Path((author, name)): Path<(String, String)>,
) -> impl IntoResponse {
    let id = format!("{author}/{name}");
    Json(service::detail_for_id(&state.index, &id))
}