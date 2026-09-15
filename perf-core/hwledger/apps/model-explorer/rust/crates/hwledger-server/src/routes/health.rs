//! `GET /healthz` — liveness + tiny metadata probe.
//!
//! Returns `{ "status": "ok", "data_dir": "..." }`. Intentionally
//! dependency-free: the health probe must work even when the tantivy
//! index is missing/corrupt so an orchestrator can use it to decide
//! whether to restart the pod.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};

use crate::AppState;

/// Mount the `health` routes onto an Axum [`Router`].
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/healthz", get(healthz))
}

/// `GET /healthz` — 200 OK with a tiny JSON body.
async fn healthz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let body = serde_json::json!({
        "status": "ok",
        "data_dir": state.data_dir.display().to_string(),
    });
    (StatusCode::OK, Json(body))
}