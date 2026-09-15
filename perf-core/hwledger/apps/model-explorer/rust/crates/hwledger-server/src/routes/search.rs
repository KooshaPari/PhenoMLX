//! `POST /v1/search` — BM25 hybrid search with optional facets.
//!
//! Accepts a [`SearchRequest`] body (see [`crate::service::SearchRequest`])
//! and returns
//! `{ "query": ..., "limit": ..., "intent": ..., "facets": ...,
//!    "results": [FusedResult, ...] }`.
//!
//! After the BM25+RRF fusion in
//! [`crate::service::search_results`], the response runs through the
//! [`default_registry`](hwledger_search_skills::default_registry)
//! (`AgenticFitRerank` → `LlmSummarizer`) before being serialized.
//! Intent is auto-detected from the query text via
//! [`crate::service::detect_intent`] so callers don't have to hint it.
//! When the resolved intent is anything other than `Agentic` / `Coding`,
//! the `AgenticFitRerank` skill is a no-op pass-through (see
//! [`hwledger_search_skills::AgenticFitRerank`]).

use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};

use hwledger_search_skills::default_registry;

use crate::service::{self, SearchRequest, ServiceError};
use crate::AppState;

/// Mount the `search` routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/search", post(search))
}

/// `POST /v1/search` — run a hybrid search and rerank the result set
/// with the default [`SkillRegistry`](hwledger_search_skills::SkillRegistry).
async fn search(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SearchRequest>,
) -> Result<impl IntoResponse, ServiceError> {
    let q = req.to_query();
    let ctx = service::build_search_context(&q);

    // (1) BM25 + RRF fusion with per-result intent-fit payload attached.
    let mut results = service::search_results(&state.index, &q).await?;

    // (2) Run the skill registry (AgenticFitRerank → LlmSummarizer).
    //     The first failing skill short-circuits via `ServiceError::Core`.
    default_registry()
        .run_all(&mut results, &ctx)
        .map_err(ServiceError::from)?;

    Ok(Json(serde_json::json!({
        "query": q.text,
        "limit": q.limit,
        "intent": service::intent_label(ctx.intent),
        "facets": q.facets,
        "results": results,
    })))
}