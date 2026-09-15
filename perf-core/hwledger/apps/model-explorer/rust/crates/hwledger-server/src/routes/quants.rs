//! `GET /v1/models/{author}/{name}/quants` — quantization tags for one model.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};

use crate::service;
use crate::AppState;

/// Mount the `quants` routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/models/:author/:name/quants", get(quants))
}

/// `GET /v1/models/:author/:name/quants` — quant list.
async fn quants(
    State(state): State<Arc<AppState>>,
    Path((author, name)): Path<(String, String)>,
) -> impl IntoResponse {
    let id = format!("{author}/{name}");
    let q = service::quants_for_id(&state.index, &id);
    Json(serde_json::json!({
        "id": id,
        "quants": q,
    }))
}