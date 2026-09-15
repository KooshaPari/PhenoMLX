//! Admin endpoints.
//!
//! Every route mounted under `/v1/admin/*` is gated by
//! [`crate::service::check_admin_token`], which compares the inbound
//! `x-admin-token` header against the `ADMIN_TOKEN` env var. When
//! `ADMIN_TOKEN` is unset the admin endpoints reject *every* request
//! with `401` — the alternative (silently granting access when the env
//! var is unset) is a footgun in production.
//!
//! Today the admin surface is intentionally tiny:
//!
//! - `POST /v1/admin/collapse` — apply the variant-collapse rule to a
//!   supplied BM25 hit slice. Useful for offline evaluation harnesses
//!   that want to compare different collapse rules against a saved
//!   search-result blob.

use std::sync::Arc;

use axum::{
    extract::State,
    http::HeaderMap,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::Deserialize;

use hwledger_search_index::IndexHit;

use crate::service::{self, ServiceError};
use crate::AppState;

/// Mount the admin routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/admin/collapse", post(collapse))
}

/// Body for `POST /v1/admin/collapse`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CollapseRequest {
    /// The BM25 hits to collapse. Each row is `{ id: string, score: number }`.
    #[serde(default)]
    pub hits: Vec<IndexHit>,
}

/// `POST /v1/admin/collapse` — collapse variants in a hit slice.
///
/// Requires a valid `x-admin-token` header matching the `ADMIN_TOKEN`
/// environment variable.
async fn collapse(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CollapseRequest>,
) -> Result<impl IntoResponse, ServiceError> {
    service::check_admin_token(&headers)?;
    let groups = service::collapse(req.hits);
    Ok(Json(serde_json::json!({
        "data_dir": state.data_dir.display().to_string(),
        "groups": groups,
    })))
}