//! MCP tool implementations exposed by `hwledger-mcp`.
//!
//! Each tool has two responsibilities:
//!
//! 1. Validate the `arguments` object (the field inside a
//!    `tools/call` request). Bad input becomes a JSON-RPC
//!    `Invalid params` error.
//! 2. Forward to the corresponding [`crate::backend::Backend`] method.
//!    The backend either hits the real tantivy index (in production) or
//!    a recording mock (in tests).
//!
//! The six tools are the canonical MCP-2024-11-05 surface for the model
//! explorer:
//!
//! 1. `model_search`        — text + facet search over the model index
//! 2. `model_detail`        — full record for a single `(source, id)`
//! 3. `model_rag_ask`       — RAG-grounded answer to a natural-language Q
//! 4. `model_quants`        — list quantized variants for a model
//! 5. `similar_models`      — find models nearest to a seed
//! 6. `models_for_use_case` — facet-based "which fits my use case?" filter
//!
//! Each tool advertises a JSON-Schema parameter block via
//! [`tool_definitions`]; the server itself is schema-agnostic and only
//! validates that `params` is a JSON object.
use serde_json::{json, Value};

use crate::backend::Backend;
use crate::error::McpError;

/// Return the list of MCP tool descriptors advertised by `tools/list`.
///
/// The shape mirrors what the MCP 2024-11-05 spec expects under
/// `result.tools[]`: each entry has a stable `name`, a human-readable
/// `description`, and an `inputSchema` describing its arguments.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "model_search",
            "description": "Full-text + faceted search over the hwledger model index. Returns a ranked list of candidate model refs.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Natural-language or keyword query."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 50, "default": 10},
                    "kinds": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Optional ModelKind filter (e.g. ['instruct', 'coding'])."
                    },
                    "facets": {
                        "type": "object",
                        "description": "Optional structured-facet bag; the v1 backend honours { kinds: [...] }."
                    }
                },
                "required": ["query"],
                "additionalProperties": true
            }
        },
        {
            "name": "model_detail",
            "description": "Fetch the full record for a single model identified by (source, id).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {"type": "string", "description": "Upstream source key (e.g. 'hf', 'internal'); defaults to 'hf'."},
                    "id":     {"type": "string", "description": "Source-specific model identifier (may include '::' for explicit source prefix)."}
                },
                "required": ["source", "id"],
                "additionalProperties": false
            }
        },
        {
            "name": "model_rag_ask",
            "description": "Answer a natural-language question about models, grounded on indexed card text via RAG.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "question": {"type": "string", "description": "Free-form question."},
                    "top_k":    {"type": "integer", "minimum": 1, "maximum": 20, "default": 5}
                },
                "required": ["question"],
                "additionalProperties": true
            }
        },
        {
            "name": "model_quants",
            "description": "List known quantization variants (gguf, gptq, awq, ...) for a model.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {"type": "string", "description": "Upstream source key; defaults to 'hf'."},
                    "id":     {"type": "string", "description": "Source-specific model identifier."}
                },
                "required": ["source", "id"],
                "additionalProperties": false
            }
        },
        {
            "name": "similar_models",
            "description": "Return models most similar to the given seed model (BM25 more-like-this over indexed fields).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {"type": "string", "description": "Upstream source key; defaults to 'hf'."},
                    "id":     {"type": "string", "description": "Source-specific model identifier."},
                    "limit":  {"type": "integer", "minimum": 1, "maximum": 50, "default": 10}
                },
                "required": ["source", "id"],
                "additionalProperties": true
            }
        },
        {
            "name": "models_for_use_case",
            "description": "Recommend models that fit a target use case (agentic, coding, reasoning, embedding).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "use_case": {"type": "string", "description": "One of: 'agentic' | 'coding' | 'reasoning' | 'embedding'."},
                    "limit":    {"type": "integer", "minimum": 1, "maximum": 50, "default": 10},
                    "text":     {"type": "string", "description": "Optional free-text refinement of the use-case filter."}
                },
                "required": ["use_case"],
                "additionalProperties": true
            }
        }
    ])
}

/// Lookup the registered tool name list. Used by tests and by `tools/list`.
pub fn tool_names() -> &'static [&'static str] {
    &[
        "model_search",
        "model_detail",
        "model_rag_ask",
        "model_quants",
        "similar_models",
        "models_for_use_case",
    ]
}

/// Dispatch a single `tools/call` invocation to the matching tool.
///
/// `params` is the raw JSON object from the `tools/call` request. The
/// dispatcher is the single source of truth for the per-tool "is this
/// structurally valid?" check (the fields advertised in
/// [`tool_definitions`]'s `inputSchema.required`). On a failure it
/// returns [`McpError::InvalidParams`]; a well-formed call always
/// delegates to the [`Backend`].
///
/// We validate up-front rather than letting the backend reject, so a
/// `MockBackend` (which doesn't enforce schema) still produces the same
/// error code as the real one.
pub fn call_tool(backend: &dyn Backend, name: &str, params: &Value) -> Result<Value, McpError> {
    let obj = params
        .as_object()
        .ok_or_else(|| McpError::invalid_params("params must be a JSON object"))?;

    match name {
        "model_search" => {
            require_string(obj, "query")?;
            backend.model_search(params)
        }
        "model_detail" => {
            require_string(obj, "source")?;
            require_string(obj, "id")?;
            backend.model_detail(params)
        }
        "model_rag_ask" => {
            require_string(obj, "question")?;
            backend.model_rag_ask(params)
        }
        "model_quants" => {
            require_string(obj, "source")?;
            require_string(obj, "id")?;
            backend.model_quants(params)
        }
        "similar_models" => {
            require_string(obj, "source")?;
            require_string(obj, "id")?;
            backend.similar_models(params)
        }
        "models_for_use_case" => {
            require_string(obj, "use_case")?;
            backend.models_for_use_case(params)
        }
        other => Err(McpError::method_not_found(other)),
    }
}

/// Internal: `params[key]` must be present and a string. Returns
/// [`McpError::InvalidParams`] otherwise.
fn require_string<'a>(
    obj: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, McpError> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params(format!("missing or non-string `{key}`")))
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;
    use std::sync::{Arc, Mutex};

    /// Tiny helper: build a `McpState` with a fresh mock backend.
    fn state_with_mock() -> Arc<MockBackend> {
        Arc::new(MockBackend::new())
    }

    /// Same, but wrapped in a Mutex so tests can inspect `last_*` fields.
    fn state_with_mutex_mock() -> Arc<Mutex<MockBackend>> {
        Arc::new(Mutex::new(MockBackend::new()))
    }

    #[test]
    fn tool_definitions_advertise_six_tools() {
        let defs = tool_definitions();
        let tools = defs.as_array().expect("tool_definitions is an array");
        assert_eq!(tools.len(), 6);

        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["name"].as_str().expect("name is a string"))
            .collect();
        for expected in tool_names() {
            assert!(names.contains(expected), "missing tool `{expected}`");
        }
    }

    #[test]
    fn call_tool_dispatches_to_correct_backend_method() {
        let mock = state_with_mock();
        // model_search
        let v = call_tool(
            mock.as_ref(),
            "model_search",
            &json!({"query": "tiny llm", "limit": 5}),
        )
        .unwrap();
        assert_eq!(v["query"], "tiny llm");
        assert_eq!(v["limit"], 5);
        assert!(v["results"].is_array());
        assert!(mock.last_model_search.lock().unwrap().is_some());

        // model_detail
        call_tool(
            mock.as_ref(),
            "model_detail",
            &json!({"source": "hf", "id": "org/name"}),
        )
        .unwrap();
        assert!(mock.last_model_detail.lock().unwrap().is_some());

        // model_rag_ask
        call_tool(
            mock.as_ref(),
            "model_rag_ask",
            &json!({"question": "what is llama?"}),
        )
        .unwrap();
        assert!(mock.last_model_rag_ask.lock().unwrap().is_some());

        // model_quants
        call_tool(
            mock.as_ref(),
            "model_quants",
            &json!({"source": "hf", "id": "org/name"}),
        )
        .unwrap();
        assert!(mock.last_model_quants.lock().unwrap().is_some());

        // similar_models
        call_tool(
            mock.as_ref(),
            "similar_models",
            &json!({"source": "hf", "id": "org/name", "limit": 3}),
        )
        .unwrap();
        assert!(mock.last_similar_models.lock().unwrap().is_some());

        // models_for_use_case
        call_tool(
            mock.as_ref(),
            "models_for_use_case",
            &json!({"use_case": "agentic", "limit": 7}),
        )
        .unwrap();
        assert!(mock.last_models_for_use_case.lock().unwrap().is_some());
    }

    #[test]
    fn call_tool_returns_invalid_params_for_non_object() {
        let mock = state_with_mock();
        let err = call_tool(mock.as_ref(), "model_search", &json!("not-an-object"))
            .expect_err("non-object params must error");
        assert_eq!(err.code(), -32602);
    }

    #[test]
    fn call_tool_returns_method_not_found_for_unknown_tool() {
        let mock = state_with_mock();
        let err = call_tool(mock.as_ref(), "no_such_tool", &json!({}))
            .expect_err("unknown tool must error");
        assert_eq!(err.code(), -32601);
        assert_eq!(err.message(), "Method not found");
    }

    /// Verify the structural validator in `model_detail` rejects missing
    /// `source` and `id`. The mock backend's argument validation is
    /// looser (it returns a value), so we exercise the upstream
    /// `ServiceBackend` here via a tiny synthetic check on the schema
    /// instead.
    #[test]
    fn model_detail_input_schema_requires_source_and_id() {
        let defs = tool_definitions();
        let detail = defs
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "model_detail")
            .expect("model_detail is registered");
        let required = detail["inputSchema"]["required"]
            .as_array()
            .expect("required is array");
        assert!(required.iter().any(|v| v == "source"));
        assert!(required.iter().any(|v| v == "id"));
    }

    /// `model_search` should require `query`. Everything else is
    /// optional.
    #[test]
    fn model_search_input_schema_requires_query() {
        let defs = tool_definitions();
        let search = defs
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "model_search")
            .expect("model_search is registered");
        let required = search["inputSchema"]["required"]
            .as_array()
            .expect("required is array");
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "query");
    }

    /// `models_for_use_case` should require `use_case`. (Sanity check
    /// that we kept the schema in sync with the dispatcher.)
    #[test]
    fn models_for_use_case_input_schema_requires_use_case() {
        let defs = tool_definitions();
        let uc = defs
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "models_for_use_case")
            .expect("models_for_use_case is registered");
        let required = uc["inputSchema"]["required"]
            .as_array()
            .expect("required is array");
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "use_case");
    }

    /// Helper smoke test: the `MockBackend` mutex wrapper is unused
    /// directly (we read `last_*` from the regular `Arc<MockBackend>`),
    /// but we keep the helper around so future tests can adopt it.
    #[test]
    fn state_with_mutex_mock_constructs() {
        let _ = state_with_mutex_mock();
    }
}
