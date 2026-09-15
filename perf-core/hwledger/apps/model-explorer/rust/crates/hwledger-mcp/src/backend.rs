//! `Backend` — the abstraction between the MCP tool layer and the storage
//! stack.
//!
//! Every MCP tool (`model_search`, `model_detail`, …) ultimately resolves
//! to a call on a [`Backend`]. The real implementation,
//! [`ServiceBackend`], delegates each call to the canonical
//! `hwledger_server::service` function of the same name, which in turn
//! drives a [`hwledger_search_index::TantivyStore`]. A second impl, the
//! in-test [`MockBackend`], records the calls so the JSON-RPC envelope
//! can be exercised without a live tantivy index.
//!
//! ## Why sync?
//!
//! The stdio transport ([`crate::transport`]) reads one line at a time and
//! drives [`crate::McpServer::dispatch`] from a synchronous `for line in
//! reader.lines()` loop. Bridging that into the async service layer would
//! require restructuring the transport around `tokio::io`, which is
//! overkill for a server that does at most a handful of DB hits per
//! request. Instead, the [`ServiceBackend`] owns a dedicated
//! `tokio::runtime::Runtime` and uses `Runtime::block_on` per call — the
//! cost is one runtime startup and N block-on hops over the lifetime of
//! the process, both of which are negligible.
use std::sync::Arc;

use hwledger_search_index::TantivyStore;
use serde_json::{json, Value};
use tokio::runtime::Runtime;

use crate::error::McpError;

/// The six operations the MCP tool surface advertises.
///
/// Every method takes a JSON `args` object (the `arguments` field of a
/// `tools/call` request) and returns the JSON shape the spec wraps in
/// `content[0].text`. Validation of `args` is the tool's responsibility —
/// the [`Backend`] impl can assume a well-formed object.
pub trait Backend: Send + Sync {
    /// Free-text + faceted search. Mirrors `POST /v1/search`.
    fn model_search(&self, args: &Value) -> Result<Value, McpError>;

    /// Single-model lookup. Mirrors `GET /v1/models/{author}/{name}`.
    fn model_detail(&self, args: &Value) -> Result<Value, McpError>;

    /// List quantization tags recorded at index time. Mirrors
    /// `GET /v1/models/{author}/{name}/quants`.
    fn model_quants(&self, args: &Value) -> Result<Value, McpError>;

    /// Natural-language question → top-K context. Mirrors `POST /v1/ask`.
    /// Today the upstream service layer returns a stub answer; we forward
    /// it verbatim.
    fn model_rag_ask(&self, args: &Value) -> Result<Value, McpError>;

    /// "More like this". Mirrors `GET /v1/models/{author}/{name}/similar`.
    fn similar_models(&self, args: &Value) -> Result<Value, McpError>;

    /// Use-case facet filter. Mirrors `POST /v1/for-use-case`.
    fn models_for_use_case(&self, args: &Value) -> Result<Value, McpError>;
}

/// Real implementation that delegates to
/// `hwledger_server::service::*` against a tantivy store.
///
/// The owned [`Runtime`] is the bridge that lets a synchronous call site
/// (the stdio transport) drive an `async fn` (the service layer). We
/// deliberately use a `Runtime` rather than a `Handle::current()` so the
/// backend is self-contained — it can be constructed and exercised from
/// any thread, including a synchronous test harness.
pub struct ServiceBackend {
    /// Tantivy BM25 store; cheap to clone (Arc internally).
    store: Arc<TantivyStore>,
    /// Dedicated runtime for service calls.
    runtime: Runtime,
}

impl std::fmt::Debug for ServiceBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceBackend").finish_non_exhaustive()
    }
}

impl ServiceBackend {
    /// Wrap an already-opened tantivy store in a [`ServiceBackend`].
    ///
    /// The runtime is created lazily here so test code that builds a
    /// backend on every assertion doesn't pay the cost more than once.
    /// `Runtime::block_on` per call is the deliberate price of keeping
    /// the stdio transport synchronous.
    pub fn new(store: Arc<TantivyStore>) -> Result<Self, McpError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("hwledger-mcp-svc")
            .build()
            .map_err(|e| McpError::internal(format!("failed to build tokio runtime: {e}")))?;
        Ok(Self { store, runtime })
    }

    /// Borrow the underlying tantivy store (handy for tests / admin
    /// routes that want to upsert a doc without re-opening the index).
    #[must_use]
    pub fn store(&self) -> &TantivyStore {
        &self.store
    }

    /// Block on an async service call. This is the single bridge point
    /// between the sync tool layer and the async service layer.
    fn drive<F, T>(&self, fut: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.runtime.block_on(fut)
    }
}

/// Convert a [`hwledger_server::service::ServiceError`] into the MCP
/// error envelope the JSON-RPC layer expects.
///
/// `ServiceError` already carries enough information to map cleanly:
/// `NotFound` → `InvalidParams` (the MCP caller asked for something that
/// doesn't exist), everything else → `Internal`. We surface the original
/// `Display` text in the `data` field so the LLM client can log it.
fn map_service_err(e: hwledger_server::service::ServiceError) -> McpError {
    use hwledger_server::service::ServiceError;
    match e {
        ServiceError::InvalidId(_) | ServiceError::BadRequest(_) => {
            McpError::invalid_params(e.to_string())
        }
        ServiceError::NotFound(_) => McpError::invalid_params(e.to_string()),
        ServiceError::Unauthorized
        | ServiceError::Index(_)
        | ServiceError::Core(_) => McpError::internal(e.to_string()),
    }
}

impl Backend for ServiceBackend {
    fn model_search(&self, args: &Value) -> Result<Value, McpError> {
        // Argument validation happens in `tools.rs`; here we trust the
        // well-formed shape (`query: string`, `limit?: int`, `facets?`).
        let obj = args.as_object().ok_or_else(|| {
            McpError::invalid_params("`model_search` arguments must be a JSON object")
        })?;

        let query_text = obj
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("`query` must be a string"))?
            .to_string();
        let limit: usize = obj
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| usize::try_from(n).unwrap_or(usize::MAX))
            .unwrap_or(10)
            .max(1);

        // Optional kinds: parse the request's `facets.kinds` array first,
        // then fall back to the legacy top-level `kinds` field so older
        // clients that haven't migrated still work.
        let mut kinds: Vec<hwledger_search_core::ModelKind> = Vec::new();
        if let Some(arr) = obj
            .get("facets")
            .and_then(|f| f.get("kinds"))
            .and_then(|k| k.as_array())
        {
            for v in arr {
                if let Some(s) = v.as_str() {
                    if let Some(k) = parse_kind_loose(s) {
                        if !kinds.contains(&k) {
                            kinds.push(k);
                        }
                    }
                }
            }
        }
        if let Some(arr) = obj.get("kinds").and_then(|k| k.as_array()) {
            for v in arr {
                if let Some(s) = v.as_str() {
                    if let Some(k) = parse_kind_loose(s) {
                        if !kinds.contains(&k) {
                            kinds.push(k);
                        }
                    }
                }
            }
        }

        let facets = hwledger_search_core::Facets {
            kinds,
            ..Default::default()
        };
        let q = hwledger_search_core::Query {
            text: query_text.clone(),
            facets,
            sort: None,
            limit,
        };

        let results = self
            .drive(hwledger_server::service::run_hybrid(&self.store, &q))
            .map_err(map_service_err)?;

        Ok(serde_json::json!({
            "query": query_text,
            "limit": limit,
            "results": results,
            "total": results.len(),
        }))
    }

    fn model_detail(&self, args: &Value) -> Result<Value, McpError> {
        let obj = args.as_object().ok_or_else(|| {
            McpError::invalid_params("`model_detail` arguments must be a JSON object")
        })?;
        let source = obj
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("`source` must be a string"))?;
        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("`id` must be a string"))?;
        if id.is_empty() {
            return Err(McpError::invalid_params("`id` must not be empty"));
        }

        // The service layer expects a canonical `source::id`; honour the
        // caller's `source` argument when `id` doesn't already carry a
        // `::` prefix.
        let canonical = canonicalize_id(source, id);

        // `detail_for_id` is intentionally non-async / infallible: it
        // returns a `serde_json::Value` envelope with a `found` flag the
        // caller can branch on. We just forward it.
        let detail = hwledger_server::service::detail_for_id(&self.store, &canonical);

        // The upstream envelope is `{ id, found, score, kind, quants }`;
        // surface the caller's original `source` / `id` so the JSON is
        // trivially round-trippable.
        Ok(serde_json::json!({
            "source": source,
            "id": id,
            "canonical_id": canonical,
            "detail": detail,
        }))
    }

    fn model_quants(&self, args: &Value) -> Result<Value, McpError> {
        let obj = args.as_object().ok_or_else(|| {
            McpError::invalid_params("`model_quants` arguments must be a JSON object")
        })?;
        let source = obj
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("`source` must be a string"))?;
        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("`id` must be a string"))?;
        if id.is_empty() {
            return Err(McpError::invalid_params("`id` must not be empty"));
        }
        let canonical = canonicalize_id(source, id);
        let quants = hwledger_server::service::quants_for_id(&self.store, &canonical);
        Ok(serde_json::json!({
            "source": source,
            "id": id,
            "canonical_id": canonical,
            "quants": quants,
        }))
    }

    fn model_rag_ask(&self, args: &Value) -> Result<Value, McpError> {
        let obj = args.as_object().ok_or_else(|| {
            McpError::invalid_params("`model_rag_ask` arguments must be a JSON object")
        })?;
        let question = obj
            .get("question")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("`question` must be a string"))?;
        if question.is_empty() {
            return Err(McpError::invalid_params("`question` must not be empty"));
        }
        let limit: usize = obj
            .get("top_k")
            .and_then(|v| v.as_u64())
            .map(|n| usize::try_from(n).unwrap_or(usize::MAX))
            .unwrap_or(5)
            .max(1);

        // `service::ask` is the v1 RAG stub — it returns the top-K BM25
        // hits plus a `(stub)` answer string. The MCP surface forwards
        // it verbatim; the data-pipeline-driven real answer will land
        // when the RAG crate is wired in.
        let body = self
            .drive(hwledger_server::service::ask(&self.store, question, limit))
            .map_err(map_service_err)?;
        Ok(body)
    }

    fn similar_models(&self, args: &Value) -> Result<Value, McpError> {
        let obj = args.as_object().ok_or_else(|| {
            McpError::invalid_params("`similar_models` arguments must be a JSON object")
        })?;
        let source = obj
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("`source` must be a string"))?;
        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("`id` must be a string"))?;
        if id.is_empty() {
            return Err(McpError::invalid_params("`id` must not be empty"));
        }
        let limit: usize = obj
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| usize::try_from(n).unwrap_or(usize::MAX))
            .unwrap_or(10)
            .max(1);

        let canonical = canonicalize_id(source, id);
        let results = self
            .drive(hwledger_server::service::similar_to(
                &self.store,
                &canonical,
                limit,
            ))
            .map_err(map_service_err)?;

        Ok(serde_json::json!({
            "seed": canonical,
            "source": source,
            "id": id,
            "limit": limit,
            "results": results,
            "total": results.len(),
        }))
    }

    fn models_for_use_case(&self, args: &Value) -> Result<Value, McpError> {
        let obj = args.as_object().ok_or_else(|| {
            McpError::invalid_params("`models_for_use_case` arguments must be a JSON object")
        })?;
        let use_case_str = obj
            .get("use_case")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("`use_case` must be a string"))?;
        let limit: usize = obj
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| usize::try_from(n).unwrap_or(usize::MAX))
            .unwrap_or(10)
            .max(1);
        // Optional free-text refinement of the use-case filter.
        let text = obj.get("text").and_then(|v| v.as_str()).map(str::to_string);

        let use_case = parse_use_case_loose(use_case_str).ok_or_else(|| {
            let known: &'static str = "agentic | coding | reasoning | embedding";
            McpError::invalid_params(format!(
                "unknown use_case `{use_case_str}`; expected one of: {known}"
            ))
        })?;

        let results = self
            .drive(hwledger_server::service::for_use_case(
                &self.store,
                use_case,
                text.as_deref(),
                limit,
            ))
            .map_err(map_service_err)?;

        Ok(serde_json::json!({
            "use_case": use_case.as_str(),
            "text": text,
            "limit": limit,
            "results": results,
            "total": results.len(),
        }))
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Canonicalize a `(source, id)` pair into a single `source::id` string the
/// service layer can consume. If `id` already contains a `::` it's
/// returned as-is (the caller is asserting the source prefix themselves);
/// otherwise `{source}::{id}` is returned. `source` defaults to `hf`.
fn canonicalize_id(source: &str, id: &str) -> String {
    if id.contains("::") {
        id.to_string()
    } else if source.is_empty() {
        format!("hf::{id}")
    } else {
        format!("{source}::{id}")
    }
}

/// Map a lowercase string onto a [`ModelKind`]. Mirrors the parse table in
/// `hwledger_server::service::parse_kind` (which is private); unknown
/// values silently yield `None` so a stray `--kinds foo` doesn't 500 the
/// request.
fn parse_kind_loose(s: &str) -> Option<hwledger_search_core::ModelKind> {
    use hwledger_search_core::ModelKind;
    match s.to_ascii_lowercase().as_str() {
        "base" => Some(ModelKind::Base),
        "instruct" => Some(ModelKind::Instruct),
        "chat" => Some(ModelKind::Chat),
        "reasoning" => Some(ModelKind::Reasoning),
        "coding" => Some(ModelKind::Coding),
        "agentic" => Some(ModelKind::Agentic),
        "embedding" => Some(ModelKind::Embedding),
        "reranker" => Some(ModelKind::Reranker),
        "vision_language" | "vision-language" => Some(ModelKind::VisionLanguage),
        "vision_encoder" | "vision-encoder" => Some(ModelKind::VisionEncoder),
        "audio" => Some(ModelKind::Audio),
        "merge" => Some(ModelKind::Merge),
        "finetune" => Some(ModelKind::Finetune),
        "adapter" => Some(ModelKind::Adapter),
        "quant" => Some(ModelKind::Quant),
        _ => None,
    }
}

/// Map a free-form `use_case` string onto the canonical
/// [`hwledger_server::service::UseCase`] enum. Mirrors
/// `UseCase::as_str`. Unknown values yield `None`.
fn parse_use_case_loose(s: &str) -> Option<hwledger_server::service::UseCase> {
    use hwledger_server::service::UseCase;
    match s.to_ascii_lowercase().as_str() {
        "agentic" => Some(UseCase::Agentic),
        "coding" | "code" => Some(UseCase::Coding),
        "reasoning" | "reason" => Some(UseCase::Reasoning),
        "embedding" | "embed" => Some(UseCase::Embedding),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Mock backend (for tests + offline operation)
// ---------------------------------------------------------------------------

/// In-process [`Backend`] impl that records the last set of args it was
/// called with and returns a small JSON value shaped like the real
/// service's response.
///
/// Tests use this to assert that a JSON-RPC `tools/call` envelope flows
/// the right `arguments` into the right tool method without having to
/// spin up tantivy. Production code never constructs a `MockBackend`.
#[derive(Debug, Default)]
pub struct MockBackend {
    /// Last call per method; `None` until first invocation.
    pub last_model_search: std::sync::Mutex<Option<Value>>,
    /// Last call to `model_detail`.
    pub last_model_detail: std::sync::Mutex<Option<Value>>,
    /// Last call to `model_quants`.
    pub last_model_quants: std::sync::Mutex<Option<Value>>,
    /// Last call to `model_rag_ask`.
    pub last_model_rag_ask: std::sync::Mutex<Option<Value>>,
    /// Last call to `similar_models`.
    pub last_similar_models: std::sync::Mutex<Option<Value>>,
    /// Last call to `models_for_use_case`.
    pub last_models_for_use_case: std::sync::Mutex<Option<Value>>,
}

impl MockBackend {
    /// Construct an empty mock (no recorded calls yet).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Backend for MockBackend {
    fn model_search(&self, args: &Value) -> Result<Value, McpError> {
        if let Ok(mut g) = self.last_model_search.lock() {
            *g = Some(args.clone());
        }
        Ok(serde_json::json!({
            "query": args.get("query").cloned().unwrap_or(Value::Null),
            "limit": args.get("limit").cloned().unwrap_or(json!(10)),
            "results": [
                {"id": "hf::mock/example", "score": 0.5, "facets": {}}
            ],
            "total": 1,
            "stub": "mock",
        }))
    }

    fn model_detail(&self, args: &Value) -> Result<Value, McpError> {
        if let Ok(mut g) = self.last_model_detail.lock() {
            *g = Some(args.clone());
        }
        let source = args.get("source").cloned().unwrap_or(Value::Null);
        let id = args.get("id").cloned().unwrap_or(Value::Null);
        Ok(serde_json::json!({
            "source": source,
            "id": id,
            "canonical_id": format!(
                "{}::{}",
                source.as_str().unwrap_or("hf"),
                id.as_str().unwrap_or("")
            ),
            "detail": {
                "id": format!(
                    "{}::{}",
                    source.as_str().unwrap_or("hf"),
                    id.as_str().unwrap_or("")
                ),
                "found": true,
                "score": null,
                "kind": "instruct",
                "quants": ["gguf"],
            },
            "stub": "mock",
        }))
    }

    fn model_quants(&self, args: &Value) -> Result<Value, McpError> {
        if let Ok(mut g) = self.last_model_quants.lock() {
            *g = Some(args.clone());
        }
        Ok(serde_json::json!({
            "source": args.get("source").cloned().unwrap_or(Value::Null),
            "id": args.get("id").cloned().unwrap_or(Value::Null),
            "quants": ["gguf", "gptq"],
            "stub": "mock",
        }))
    }

    fn model_rag_ask(&self, args: &Value) -> Result<Value, McpError> {
        if let Ok(mut g) = self.last_model_rag_ask.lock() {
            *g = Some(args.clone());
        }
        Ok(serde_json::json!({
            "question": args.get("question").cloned().unwrap_or(Value::Null),
            "limit": args.get("top_k").cloned().unwrap_or(json!(5)),
            "answer": "[mock] no RAG pipeline wired yet",
            "context": [],
            "stub": "mock",
        }))
    }

    fn similar_models(&self, args: &Value) -> Result<Value, McpError> {
        if let Ok(mut g) = self.last_similar_models.lock() {
            *g = Some(args.clone());
        }
        Ok(serde_json::json!({
            "seed": format!(
                "{}::{}",
                args.get("source").and_then(|v| v.as_str()).unwrap_or("hf"),
                args.get("id").and_then(|v| v.as_str()).unwrap_or("")
            ),
            "limit": args.get("limit").cloned().unwrap_or(json!(10)),
            "results": [],
            "total": 0,
            "stub": "mock",
        }))
    }

    fn models_for_use_case(&self, args: &Value) -> Result<Value, McpError> {
        if let Ok(mut g) = self.last_models_for_use_case.lock() {
            *g = Some(args.clone());
        }
        Ok(serde_json::json!({
            "use_case": args.get("use_case").cloned().unwrap_or(Value::Null),
            "limit": args.get("limit").cloned().unwrap_or(json!(10)),
            "results": [],
            "total": 0,
            "stub": "mock",
        }))
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_id_prepends_source_only_when_missing() {
        assert_eq!(canonicalize_id("hf", "org/name"), "hf::org/name");
        assert_eq!(canonicalize_id("hf", "hf::org/name"), "hf::org/name");
        assert_eq!(canonicalize_id("", "org/name"), "hf::org/name");
        assert_eq!(canonicalize_id("mscope", "x/y"), "mscope::x/y");
    }

    #[test]
    fn parse_kind_loose_matches_taxonomy_strings() {
        use hwledger_search_core::ModelKind;
        assert_eq!(parse_kind_loose("instruct"), Some(ModelKind::Instruct));
        assert_eq!(
            parse_kind_loose("vision_language"),
            Some(ModelKind::VisionLanguage)
        );
        assert_eq!(
            parse_kind_loose("vision-language"),
            Some(ModelKind::VisionLanguage)
        );
        assert_eq!(parse_kind_loose("bogus"), None);
    }

    #[test]
    fn parse_use_case_loose_accepts_known_values_and_aliases() {
        use hwledger_server::service::UseCase;
        assert_eq!(parse_use_case_loose("agentic"), Some(UseCase::Agentic));
        assert_eq!(parse_use_case_loose("CODE"), Some(UseCase::Coding));
        assert_eq!(parse_use_case_loose("reason"), Some(UseCase::Reasoning));
        assert_eq!(parse_use_case_loose("embed"), Some(UseCase::Embedding));
        assert_eq!(parse_use_case_loose("nope"), None);
    }

    #[test]
    fn mock_backend_records_each_call() {
        let mock = MockBackend::new();
        mock.model_search(&serde_json::json!({"query": "q", "limit": 7}))
            .unwrap();
        mock.model_detail(&serde_json::json!({"source": "hf", "id": "o/m"}))
            .unwrap();
        mock.model_quants(&serde_json::json!({"source": "hf", "id": "o/m"}))
            .unwrap();
        mock.model_rag_ask(&serde_json::json!({"question": "q"}))
            .unwrap();
        mock.similar_models(&serde_json::json!({"source": "hf", "id": "o/m"}))
            .unwrap();
        mock.models_for_use_case(&serde_json::json!({"use_case": "agentic"}))
            .unwrap();

        assert!(mock.last_model_search.lock().unwrap().is_some());
        assert!(mock.last_model_detail.lock().unwrap().is_some());
        assert!(mock.last_model_quants.lock().unwrap().is_some());
        assert!(mock.last_model_rag_ask.lock().unwrap().is_some());
        assert!(mock.last_similar_models.lock().unwrap().is_some());
        assert!(mock.last_models_for_use_case.lock().unwrap().is_some());
    }
}
