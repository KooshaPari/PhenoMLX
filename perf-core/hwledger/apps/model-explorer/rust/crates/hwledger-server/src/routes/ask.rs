//! `POST /v1/ask` — natural-language question → top-K context (RAG v1
//! stub).
//!
//! Body: `{ question: string, limit?: number }`. Returns
//! `{ question, limit, answer, context, results }` where `answer` is the
//! v1 stub message ("top-K BM25 hits for: …") and `context` is the same
//! rows in a smaller shape. A future phase will chunk the card text +
//! run cosine retrieval here.

use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
use serde::Deserialize;

use crate::service::{self, ServiceError};
use crate::AppState;

/// Mount the `ask` routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/ask", post(ask))
}

/// Body for `POST /v1/ask`.
#[derive(Debug, Clone, Deserialize)]
pub struct AskRequest {
    /// Free-text question.
    pub question: String,
    /// Cap on returned rows.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// `POST /v1/ask` — RAG v1 stub.
async fn ask(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AskRequest>,
) -> Result<impl IntoResponse, ServiceError> {
    let limit = req.limit.unwrap_or(5);
    let body = service::ask(&state.index, &req.question, limit).await?;
    Ok(Json(body))
}