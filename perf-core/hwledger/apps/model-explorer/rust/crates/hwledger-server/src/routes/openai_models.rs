//! `GET /v1/models` — OpenAI-compatible model enumeration.
//!
//! Returns `{ "object": "list", "data": [{ "id": "..." }, ...] }` so any
//! OpenAI SDK client can call `client.models.list()` against the search
//! index. The endpoint walks the Tantivy index doc-store and emits a
//! `Model { id, object, created, owned_by }` for every indexed model id.
//!
//! Pagination: `?limit=N` (default 100, max 1000) and `?after=<id>` for
//! cursor-based iteration matching the OpenAI shape.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

/// Mount the OpenAI-compatible routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/v1/models", axum::routing::get(list))
}

#[derive(Debug, Deserialize)]
struct ListParams {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    after: Option<String>,
}

#[derive(Debug, Serialize)]
struct Model {
    id: String,
    object: &'static str,
    created: u64,
    owned_by: String,
}

#[derive(Debug, Serialize)]
struct ListResponse {
    object: &'static str,
    data: Vec<Model>,
}

/// `GET /v1/models` — OpenAI-compatible model list.
async fn list(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Json<ListResponse> {
    let limit = params.limit.unwrap_or(100).min(1000).max(1);

    // Walk the Tantivy index doc-store; this is the cheap enumeration path
    // that doesn't deserialize each model — we just need the ids.
    let mut all_ids = state.index.list_all_ids();
    all_ids.sort();

    // Cursor: skip until we see `after`, then yield.
    let mut start_pagination = true;
    let data: Vec<Model> = all_ids
        .into_iter()
        .filter_map(|id| {
            if let Some(ref cursor) = params.after {
                if start_pagination {
                    if id == *cursor {
                        start_pagination = false;
                    }
                    return None;
                }
            }
            Some(to_openai_model(&id))
        })
        .take(limit)
        .collect();

    Json(ListResponse {
        object: "list",
        data,
    })
}

fn to_openai_model(id: &str) -> Model {
    let owned_by = id
        .split_once('/')
        .map(|(org, _)| org.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    // We don't have a real created timestamp without an extra column; use
    // 0 (openai uses unix seconds; the field is informational).
    Model {
        id: id.to_string(),
        object: "model",
        created: 0,
        owned_by,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_limit_default() {
        let q: ListParams = serde_json::from_str("{}").unwrap();
        assert_eq!(q.limit, None);
    }

    #[test]
    fn extracts_owned_by_from_org_slash_name() {
        let m = to_openai_model("meta-llama/Llama-3.1-8B-Instruct");
        assert_eq!(m.id, "meta-llama/Llama-3.1-8B-Instruct");
        assert_eq!(m.owned_by, "meta-llama");
        assert_eq!(m.object, "model");
    }

    #[test]
    fn unknown_owned_by_when_no_org() {
        let m = to_openai_model("no-org");
        assert_eq!(m.owned_by, "unknown");
    }
}
