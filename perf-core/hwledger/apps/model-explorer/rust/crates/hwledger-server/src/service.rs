//! Service layer.
//!
//! Thin async wrappers around the public APIs of the `search-index`
//! crate. Handlers in `routes/` call these exclusively — they never
//! reach into the tantivy handle directly, so the service surface is the
//! only place that needs to know about the storage backend.
//!
//! Every function takes an `&Arc<TantivyStore>` so handlers can pass their
//! shared `AppState.index` without juggling lifetimes.
//!
//! All errors are funneled through [`ServiceError`] which converts cleanly
//! into an `axum::http::StatusCode` via [`ServiceError::status`].
//!
//! ## Backend choice (v1)
//!
//! `hwledger-search-index` v1 ships only `run_hybrid` (BM25 + post-filter
//! facets) and `TantivyStore::search` (raw BM25). The LanceDB dense
//! vector index lands in a later phase, at which point `run_hybrid` will
//! become truly hybrid. The route handlers in `routes/` are deliberately
//! agnostic about which backend a given query uses — they always call
//! `run_hybrid` so the migration is a no-op at the HTTP surface.

use std::sync::Arc;

use hwledger_search_core::{
    CoreError, Facets, FusedResult, ModelKind, Query, SearchContext, SearchIntent,
};
use hwledger_search_index::{
    collapse_variants as run_collapse_variants, run_hybrid as search_index_run_hybrid,
    CollapseRule, IndexError, IndexHit, TantivyStore,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can escape from the service layer.
///
/// We deliberately wrap the backend-specific `IndexError` (which lives in
/// `hwledger-search-index`) so handlers don't have to care about which
/// backend failed. [`CoreError`] (from `hwledger-search-core`) covers the
/// cross-crate primitives — `SearchSkill`s, `SourceAdapter`s — that
/// don't belong to any specific backend.
#[derive(Debug, Error)]
pub enum ServiceError {
    /// Tantivy / index backend rejected the operation.
    #[error("index error: {0}")]
    Index(#[from] IndexError),

    /// A search-core primitive (skill, source adapter) returned an
    /// unrecoverable error.
    #[error("core error: {0}")]
    Core(#[from] CoreError),

    /// The caller supplied an invalid id (e.g. empty after `hf::` prefix).
    #[error("invalid id: {0}")]
    InvalidId(String),

    /// The requested resource was not found in the index.
    #[error("not found: {0}")]
    NotFound(String),

    /// An `admin` route was hit without a valid `x-admin-token` header.
    #[error("unauthorized")]
    Unauthorized,

    /// The request body failed to (de)serialize.
    #[error("bad request: {0}")]
    BadRequest(String),
}

impl ServiceError {
    /// Map the error onto an HTTP status code.
    #[must_use]
    pub fn status(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            Self::Index(_) | Self::Core(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidId(_) | Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
        }
    }

    /// Render the error as a small JSON envelope.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "error": self.to_string() })
    }
}

impl axum::response::IntoResponse for ServiceError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status();
        let body = self.to_json();
        (status, axum::Json(body)).into_response()
    }
}

/// JSON request body for `POST /v1/search`.
///
/// Mirrors `hwledger_search_core::Query` plus an optional `kinds` shorthand
/// so HTTP callers don't need to know the snake_case serialization of
/// [`ModelKind`].
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SearchRequest {
    /// Free-text query.
    #[serde(default)]
    pub text: String,
    /// Cap on returned rows.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Optional `ModelKind` facet filter, e.g. `["instruct", "coding"]`.
    #[serde(default)]
    pub kinds: Vec<String>,
    /// Optional explicit sort key (e.g. `"downloads"`, `"agentic_fit"`).
    #[serde(default)]
    pub sort: Option<String>,
}

impl SearchRequest {
    /// Project this request into the core `Query` type.
    #[must_use]
    pub fn to_query(&self) -> Query {
        let mut facets = Facets::default();
        for raw in &self.kinds {
            if let Some(k) = parse_kind(raw) {
                if !facets.kinds.contains(&k) {
                    facets.kinds.push(k);
                }
            }
        }
        Query {
            text: self.text.clone(),
            facets,
            sort: self.sort.clone(),
            limit: self.limit.unwrap_or(25).max(1),
        }
    }
}

/// Use cases `POST /v1/for-use-case` understands today.
///
/// Mirrors the CLI's `UseCase` enum one-to-one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UseCase {
    /// Tool-using / agentic workloads.
    Agentic,
    /// Programming assistants.
    Coding,
    /// General reasoning / chain-of-thought.
    Reasoning,
    /// Embedding lookup.
    #[default]
    Embedding,
}

impl UseCase {
    /// Lowercase string form (matches the CLI).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agentic => "agentic",
            Self::Coding => "coding",
            Self::Reasoning => "reasoning",
            Self::Embedding => "embedding",
        }
    }

    /// Map to the matching [`ModelKind`] facet.
    #[must_use]
    pub fn as_kind(self) -> ModelKind {
        match self {
            Self::Agentic => ModelKind::Agentic,
            Self::Coding => ModelKind::Coding,
            Self::Reasoning => ModelKind::Reasoning,
            Self::Embedding => ModelKind::Embedding,
        }
    }
}

/// Map the lowercase string form onto a [`ModelKind`]. Unknown values
/// silently yield `None` so a stray `--kind foo` doesn't 500 the request.
fn parse_kind(s: &str) -> Option<ModelKind> {
    match s.to_ascii_lowercase().as_str() {
        "base" => Some(ModelKind::Base),
        "instruct" => Some(ModelKind::Instruct),
        "chat" => Some(ModelKind::Chat),
        "reasoning" => Some(ModelKind::Reasoning),
        "coding" => Some(ModelKind::Coding),
        "agentic" => Some(ModelKind::Agentic),
        "embedding" => Some(ModelKind::Embedding),
        "reranker" => Some(ModelKind::Reranker),
        "vision_language" => Some(ModelKind::VisionLanguage),
        "vision_encoder" => Some(ModelKind::VisionEncoder),
        "audio" => Some(ModelKind::Audio),
        "merge" => Some(ModelKind::Merge),
        "finetune" => Some(ModelKind::Finetune),
        "adapter" => Some(ModelKind::Adapter),
        "quant" => Some(ModelKind::Quant),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Read paths
// ---------------------------------------------------------------------------

/// Run a free-text + faceted hybrid search.
///
/// This is a thin async wrapper around `hwledger_search_index::run_hybrid`
/// — the v1 implementation is BM25-only with a `kinds` facet post-filter.
pub async fn run_hybrid(
    store: &Arc<TantivyStore>,
    q: &Query,
) -> Result<Vec<FusedResult>, ServiceError> {
    let k = q.limit.max(1);
    let hits = search_index_run_hybrid(store.as_ref(), q, k)
        .await
        .map_err(ServiceError::from)?;
    Ok(hits)
}

/// Run the BM25+RRF hybrid search and attach the per-result intent-fit
/// payload needed by the [`AgenticFitRerank`](hwledger_search_skills::AgenticFitRerank)
/// skill.
///
/// v1's tantivy schema doesn't yet carry an `agentic` / `coding` numeric
/// column, so we project the model's stored `kind` (which is already in
/// the sidecar cache) into the `payload` field that the skill reads from:
///
/// | doc kind | `payload.agentic` | `payload.coding` |
/// |----------|-------------------|------------------|
/// | `agentic`| `1.0`             | `0.0`            |
/// | `coding` | `0.0`             | `1.0`            |
/// | anything | `0.0`             | `0.0`            |
///
/// This is intentionally a thin projection rather than a real fit
/// estimator — once the `usecase_fit_tagger` lands it will write the
/// per-model `agentic` / `coding` numerics directly to tantivy and this
/// function will become a pass-through.
pub async fn search_results(
    store: &Arc<TantivyStore>,
    q: &Query,
) -> Result<Vec<FusedResult>, ServiceError> {
    let mut results = run_hybrid(store, q).await?;
    for r in results.iter_mut() {
        r.payload = Some(intent_fit_payload(store, &r.id));
    }
    Ok(results)
}

/// Derive the `{ agentic, coding }` JSON payload for `id` from the
/// per-id `kind` sidecar cache.
///
/// Unknown kinds yield an all-zero payload so the rerank skill treats
/// them as neutral (the policy: 0.6 * score + 0.4 * 0.0 = 0.6 * score).
fn intent_fit_payload(store: &Arc<TantivyStore>, id: &str) -> serde_json::Value {
    let kind_str = store.kind_for_id(id).unwrap_or_default();
    let kind = parse_kind(&kind_str);
    let agentic = if kind == Some(ModelKind::Agentic) { 1.0_f32 } else { 0.0_f32 };
    let coding = if kind == Some(ModelKind::Coding) { 1.0_f32 } else { 0.0_f32 };
    serde_json::json!({
        "agentic": agentic,
        "coding": coding,
    })
}

/// Classify a free-text query into a [`SearchIntent`].
///
/// This is intentionally a keyword-based heuristic — a future phase
/// will swap in an LLM-backed classifier. The keywords chosen here are
/// the ones the CLI's `--use-case` flag and `for-use-case` endpoint
/// already recognize, so a route that auto-detects the intent will
/// agree with one that was hinted explicitly.
#[must_use]
pub fn detect_intent(text: &str) -> SearchIntent {
    let lowered = text.to_ascii_lowercase();
    if lowered.contains("agent")
        || lowered.contains("tool")
        || lowered.contains("function call")
        || lowered.contains("tool-use")
    {
        return SearchIntent::Agentic;
    }
    if lowered.contains("code")
        || lowered.contains("coding")
        || lowered.contains("program")
        || lowered.contains("completion")
    {
        return SearchIntent::Coding;
    }
    if lowered.contains("reason") || lowered.contains("chain-of-thought") {
        return SearchIntent::Reasoning;
    }
    if lowered.contains("embed") {
        return SearchIntent::Embedding;
    }
    SearchIntent::Generic
}

/// Lowercase string form for a [`SearchIntent`] — used by the route
/// layer so the JSON response can surface the resolved intent without
/// requiring `SearchIntent` to derive `Serialize`.
#[must_use]
pub fn intent_label(intent: SearchIntent) -> &'static str {
    match intent {
        SearchIntent::Generic => "generic",
        SearchIntent::Coding => "coding",
        SearchIntent::Agentic => "agentic",
        SearchIntent::Reasoning => "reasoning",
        SearchIntent::Embedding => "embedding",
    }
}

/// Build the [`SearchContext`] (query + resolved intent) that every
/// skill in [`default_registry`](hwledger_search_skills::default_registry)
/// receives.
#[must_use]
pub fn build_search_context(q: &Query) -> SearchContext {
    SearchContext::new(q.clone(), detect_intent(&q.text))
}

/// Look up one model by id. Returns a JSON-shaped `serde_json::Value` so
/// the caller can decide how to render it.
///
/// Existence is determined by the sidecar `kind_for_id` hashmap (an exact
/// key match), not by the BM25 search — the id field is `STRING` (untokenized)
/// so it can't be matched by the multi-field query parser. We still issue
/// a search and surface its score as a `popularity` hint when the doc is
/// found.
#[must_use]
pub fn detail_for_id(
    store: &Arc<TantivyStore>,
    id: &str,
) -> serde_json::Value {
    let canonical = canonicalize_id(id);
    let kind = store.kind_for_id(&canonical);
    let quants = store.quants_for_id(&canonical).unwrap_or_default();
    let found = kind.is_some();
    let score = if found {
        store.search(&canonical, 1).unwrap_or_default().first().map(|h| h.score)
    } else {
        None
    };

    serde_json::json!({
        "id": canonical,
        "found": found,
        "score": score,
        "kind": kind,
        "quants": quants,
    })
}

/// Quantization tags recorded at index time for `id`.
#[must_use]
pub fn quants_for_id(store: &Arc<TantivyStore>, id: &str) -> Vec<String> {
    let canonical = canonicalize_id(id);
    store.quants_for_id(&canonical).unwrap_or_default()
}

/// "More like this" lookup — re-issue a BM25 query using the model's id
/// tokens, dropping the seed itself from the result.
pub async fn similar_to(
    store: &Arc<TantivyStore>,
    id: &str,
    limit: usize,
) -> Result<Vec<FusedResult>, ServiceError> {
    let canonical = canonicalize_id(id);
    let text = strip_source_prefix(&canonical).to_string();
    let q = Query {
        text,
        facets: Facets::default(),
        sort: None,
        limit: limit.max(1),
    };
    let hits = run_hybrid(store, &q).await?;
    Ok(hits.into_iter().filter(|r| r.id != canonical).collect())
}

/// Use-case filtered search. v1 short-circuits via the `ModelKind` facet;
/// a future phase will swap in `agentic_fit` / `coding_fit` numerics.
pub async fn for_use_case(
    store: &Arc<TantivyStore>,
    use_case: UseCase,
    text: Option<&str>,
    limit: usize,
) -> Result<Vec<FusedResult>, ServiceError> {
    let mut facets = Facets::default();
    facets.kinds.push(use_case.as_kind());

    let effective_text = text
        .filter(|t| !t.is_empty())
        .map_or_else(|| use_case.as_str().to_string(), ToString::to_string);

    let q = Query {
        text: effective_text,
        facets,
        sort: Some("agentic_fit".to_string()),
        limit: limit.max(1),
    };
    run_hybrid(store, &q).await
}

/// v1 RAG stub — echo the top-K BM25 hits as "context" for `question`.
pub async fn ask(
    store: &Arc<TantivyStore>,
    question: &str,
    limit: usize,
) -> Result<serde_json::Value, ServiceError> {
    let q = Query::text(question).with_limit(limit.max(1));
    let hits = run_hybrid(store, &q).await?;
    let context: Vec<serde_json::Value> = hits
        .iter()
        .map(|h| {
            serde_json::json!({
                "id": h.id,
                "score": h.score,
                "snippet": "",
            })
        })
        .collect();
    Ok(serde_json::json!({
        "question": question,
        "limit": limit,
        "answer": format!("(stub) top-{} BM25 hits for: {}", hits.len(), question),
        "context": context,
        "results": hits,
    }))
}

/// Apply the variant-collapse rule to a BM25 hit slice.
#[must_use]
pub fn collapse(hits: Vec<IndexHit>) -> Vec<hwledger_search_index::CollapsedHit> {
    run_collapse_variants(hits, &CollapseRule::default())
}

// ---------------------------------------------------------------------------
// Admin paths
// ---------------------------------------------------------------------------

use std::sync::{Mutex, OnceLock};

/// Process-local admin token store.
///
/// Read from `ADMIN_TOKEN` once at startup (see [`init_admin_token`]) and
/// stored here so handlers don't have to round-trip through `std::env::var`
/// on every request — `std::env::var` is technically thread-safe but
/// parallel-test environments that mutate it can race, so we cache the
/// resolved value in a `OnceLock`.
static ADMIN_TOKEN: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// Cache the `ADMIN_TOKEN` env var into the process-local store.
///
/// Idempotent — safe to call multiple times (e.g. from test harnesses).
pub fn init_admin_token() {
    let cell = ADMIN_TOKEN.get_or_init(|| Mutex::new(None));
    let raw = std::env::var("ADMIN_TOKEN").unwrap_or_default();
    if let Ok(mut guard) = cell.lock() {
        if raw.is_empty() {
            *guard = None;
        } else {
            *guard = Some(raw);
        }
    }
}

/// Test-only override for [`init_admin_token`].
///
/// Production code uses [`init_admin_token`]; tests use this hook to set
/// or clear the token without racing on a process-global environment
/// variable across multiple `#[tokio::test]` threads.
#[doc(hidden)]
pub fn set_admin_token_for_testing(value: Option<String>) {
    let cell = ADMIN_TOKEN.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = cell.lock() {
        *guard = value;
    }
}

/// Read the currently-configured admin token (if any).
fn current_admin_token() -> Option<String> {
    ADMIN_TOKEN
        .get()
        .and_then(|cell| cell.lock().ok().map(|g| g.clone()).unwrap_or(None))
}

/// Verify the inbound admin token against the value the server was
/// started with. Returns `Unauthorized` on mismatch or when no
/// `ADMIN_TOKEN` is configured.
pub fn check_admin_token(headers: &axum::http::HeaderMap) -> Result<(), ServiceError> {
    let configured = current_admin_token();
    let Some(configured) = configured else {
        // No token configured. Reject *every* admin request rather than
        // implicitly allowing them — the alternative (silently granting
        // access when ADMIN_TOKEN is unset) is a footgun in production
        // deployments where the operator forgot to set it.
        return Err(ServiceError::Unauthorized);
    };
    let supplied = headers
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if supplied == configured {
        Ok(())
    } else {
        Err(ServiceError::Unauthorized)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Turn `org/name` into `hf::org/name` if no source prefix is present.
fn canonicalize_id(id: &str) -> String {
    if id.is_empty() {
        return String::new();
    }
    if id.contains("::") {
        id.to_string()
    } else {
        format!("hf::{id}")
    }
}

/// Strip the leading `source::` portion of a key.
fn strip_source_prefix(id: &str) -> &str {
    match id.find("::") {
        Some(idx) => &id[idx + 2..],
        None => id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_error_status_mapping() {
        use axum::http::StatusCode;
        assert_eq!(
            ServiceError::InvalidId("x".into()).status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ServiceError::NotFound("x".into()).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(ServiceError::Unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            ServiceError::BadRequest("x".into()).status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn search_request_to_query_propagates_kinds_and_limit() {
        let req = SearchRequest {
            text: "instruct".into(),
            limit: Some(7),
            kinds: vec!["instruct".into(), "coding".into(), "bogus".into()],
            sort: None,
        };
        let q = req.to_query();
        assert_eq!(q.text, "instruct");
        assert_eq!(q.limit, 7);
        // `bogus` is dropped silently.
        assert_eq!(q.facets.kinds.len(), 2);
        assert!(q.facets.kinds.contains(&ModelKind::Instruct));
        assert!(q.facets.kinds.contains(&ModelKind::Coding));
    }

    #[test]
    fn search_request_default_limit_is_25() {
        let req = SearchRequest::default();
        let q = req.to_query();
        assert_eq!(q.limit, 25);
    }

    #[test]
    fn use_case_kind_round_trip() {
        for uc in [UseCase::Agentic, UseCase::Coding, UseCase::Reasoning, UseCase::Embedding] {
            assert_eq!(uc.as_str(), serde_json::to_string(&uc).unwrap().trim_matches('"'));
        }
    }

    #[test]
    fn canonicalize_id_adds_hf_prefix() {
        assert_eq!(canonicalize_id("org/name"), "hf::org/name");
        assert_eq!(canonicalize_id("hf::org/name"), "hf::org/name");
        assert_eq!(canonicalize_id("mscope::x/y"), "mscope::x/y");
    }

    #[test]
    fn detect_intent_classifies_agentic_keywords() {
        assert_eq!(detect_intent("agent for tool use"), SearchIntent::Agentic);
        assert_eq!(detect_intent("function calling assistant"), SearchIntent::Agentic);
        assert_eq!(detect_intent("tool-use ready model"), SearchIntent::Agentic);
    }

    #[test]
    fn detect_intent_classifies_coding_keywords() {
        assert_eq!(detect_intent("code completion model"), SearchIntent::Coding);
        assert_eq!(detect_intent("programming helper"), SearchIntent::Coding);
        assert_eq!(detect_intent("Coding assistant"), SearchIntent::Coding);
    }

    #[test]
    fn detect_intent_falls_through_to_generic_when_no_keyword_matches() {
        assert_eq!(detect_intent(""), SearchIntent::Generic);
        assert_eq!(detect_intent("instruct"), SearchIntent::Generic);
        assert_eq!(detect_intent("embedding model please"), SearchIntent::Embedding);
        assert_eq!(detect_intent("reasoning chain-of-thought"), SearchIntent::Reasoning);
    }

    #[test]
    fn intent_label_round_trip() {
        assert_eq!(intent_label(SearchIntent::Generic), "generic");
        assert_eq!(intent_label(SearchIntent::Coding), "coding");
        assert_eq!(intent_label(SearchIntent::Agentic), "agentic");
        assert_eq!(intent_label(SearchIntent::Reasoning), "reasoning");
        assert_eq!(intent_label(SearchIntent::Embedding), "embedding");
    }
}