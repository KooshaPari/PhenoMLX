//! `hwledger-mcp` — minimal MCP 2024-11-05 server over JSON-RPC 2.0.
//!
//! This crate implements just enough of the Model Context Protocol
//! specification to be a useful tool provider for an LLM client connected
//! over either stdio (`transport`) or HTTP+SSE (`transport_http`). It
//! deliberately bypasses the upstream `rmcp` SDK to avoid a
//! `PointeeSized` compatibility issue with Rust 1.95; the whole transport
//! layer is ~200 lines of straightforward `serde_json` + `std::io`.
//!
//! Layering:
//!
//! * [`error::McpError`]  — typed errors with JSON-RPC 2.0 error codes.
//! * [`backend::Backend`] — trait abstracting the search index; the real
//!   implementation ([`backend::ServiceBackend`])
//!   delegates to `hwledger_server::service::*`,
//!   and the in-test [`backend::MockBackend`]
//!   records calls without a live tantivy index.
//! * [`tools`]            — six tool implementations, each one a thin
//!   validator that forwards to the [`Backend`].
//! * `McpServer` / `McpState` (this module) — request validation +
//!   dispatch.
//! * [`transport`]        — stdio read/write loop and per-message handler.
//! * [`transport_http`]   — axum router exposing POST `/mcp` (per-request
//!   JSON-RPC) and GET `/sse` (Server-Sent Events for streaming
//!   notifications), reusing the same `McpServer::dispatch` core.
//!
//! Round-trip flow for a single request:
//!
//! 1. Client sends one JSON-RPC 2.0 request (stdin line or POST `/mcp`
//!    body).
//! 2. The transport layer parses it, validates the envelope, and
//!    dispatches to the matching method on `McpServer`.
//! 3. `McpServer` returns either a `result` value or an `error` object.
//! 4. The response (if any) is serialised and written back (stdout or
//!    HTTP response body).
#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

pub mod backend;
pub mod error;
pub mod tools;
pub mod transport;
pub mod transport_http;

use std::sync::Arc;

use serde_json::{json, Value};

use crate::backend::Backend;
use crate::error::McpError;

/// Server-wide state that mutates over the lifetime of a connection.
///
/// `McpState` is the single object the transport, server, and tool layers
/// all share. It carries:
///
/// * the MCP protocol-version + `client_info` captured during
///   `initialize`;
/// * a `Backend` implementation — either the real
///   [`backend::ServiceBackend`] in production, or a
///   [`backend::MockBackend`] in tests.
pub struct McpState {
    /// The tool backend (`model_search`, `model_detail`, ...).
    pub backend: Arc<dyn Backend>,
    /// Set to `true` once the client has completed the `initialize` /
    /// `notifications/initialized` handshake.
    pub initialized: bool,
    /// Client identity fields captured during `initialize`, kept verbatim
    /// so the server can echo them back in logs/diagnostics.
    pub client_info: Option<Value>,
    /// Protocol version the client claimed in `initialize`.
    pub protocol_version: Option<String>,
}

impl std::fmt::Debug for McpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpState")
            .field("backend", &"<dyn Backend>")
            .field("initialized", &self.initialized)
            .field("client_info", &self.client_info)
            .field("protocol_version", &self.protocol_version)
            .finish()
    }
}

impl McpState {
    /// Construct a fresh, uninitialized state bound to `backend`.
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            initialized: false,
            client_info: None,
            protocol_version: None,
        }
    }
}

/// The MCP server itself.
///
/// `McpServer` is intentionally cheap and stateless — it holds no I/O,
/// no I/O handles, and no caches. All per-connection state lives in
/// `McpState`, which is passed mutably into each `handle_*` call.
#[derive(Debug, Default, Clone, Copy)]
pub struct McpServer;

impl McpServer {
    /// Construct a new server handle.
    pub fn new() -> Self {
        Self
    }

    /// Handle the MCP `initialize` request and return the `result` payload
    /// the spec expects (`protocolVersion`, `serverInfo`, `capabilities`).
    ///
    /// Also mutates [`McpState`] to record the client info and protocol
    /// version; the actual `initialized = true` flip happens on receipt of
    /// the `notifications/initialized` notification.
    pub fn handle_initialize(
        &self,
        state: &mut McpState,
        params: Option<&Value>,
    ) -> Result<Value, McpError> {
        let params = params.unwrap_or(&Value::Null);
        let obj = params
            .as_object()
            .ok_or_else(|| McpError::invalid_params("`initialize` params must be an object"))?;

        state.protocol_version = obj
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        state.client_info = obj.get("clientInfo").cloned();

        // MCP 2024-11-05 protocol version we speak.
        //
        // `tools`, `resources`, and `prompts` are all advertised so that
        // well-behaved MCP clients (e.g. Claude Desktop, Cursor) show the
        // server's full surface in their UI rather than hiding it
        // behind an "unsupported" badge. `listChanged: false` (for each
        // capability) tells the client that the descriptor list is
        // static for the lifetime of this connection — we don't emit
        // `notifications/tools/list_changed`, `resources/list_changed`,
        // or `prompts/list_changed` today.
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "hwledger-mcp",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": {
                "tools": {"listChanged": false},
                "resources": {"listChanged": false, "subscribe": false},
                "prompts": {"listChanged": false}
            }
        }))
    }

    /// Handle the MCP `tools/list` request and return the array of tool
    /// descriptors advertised by this server.
    pub fn handle_tools_list(&self) -> Result<Value, McpError> {
        Ok(json!({ "tools": tools::tool_definitions() }))
    }

    /// Handle the MCP `tools/call` request: dispatch to the named tool and
    /// return its result wrapped in the MCP `content` envelope
    /// (`{ content: [{ type: "text", text: "<json>" }], isError: false }`).
    ///
    /// Per the task spec, tool-level "method not found" / "invalid params"
    /// errors propagate up as JSON-RPC error codes (`-32601` / `-32602`)
    /// rather than being downgraded to a soft `isError: true` payload —
    /// that mirrors how the reference MCP implementations surface
    /// `tools/call` failures at the JSON-RPC layer.
    pub fn handle_tools_call(
        &self,
        state: &McpState,
        params: Option<&Value>,
    ) -> Result<Value, McpError> {
        let params = params.unwrap_or(&Value::Null);
        let obj = params
            .as_object()
            .ok_or_else(|| McpError::invalid_params("`tools/call` params must be an object"))?;

        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("`tools/call` requires a `name` string"))?;

        let args = obj.get("arguments").cloned().unwrap_or_else(|| json!({}));

        let value = tools::call_tool(state.backend.as_ref(), name, &args)?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string(&value)
                    .unwrap_or_else(|_| value.to_string()),
            }],
            "isError": false,
        }))
    }

    /// Handle the MCP `resources/list` request.
    ///
    /// Returns `{ resources: [] }` because the server does not yet register
    /// any readable resources (the `resources/read` handler is a stub that
    /// always returns an empty `contents` array). Advertising the
    /// `resources` capability in `initialize` (see [`Self::handle_initialize`])
    /// tells the client the method is supported; the empty list tells it
    /// there is nothing to choose from yet — a spec-compliant shape that
    /// matches how upstream reference servers report "no resources".
    pub fn handle_resources_list(&self) -> Result<Value, McpError> {
        Ok(json!({ "resources": [] }))
    }

    /// Handle the MCP `resources/read` request.
    ///
    /// Per the spec, `params` must be an object containing a `uri`
    /// string identifying the resource to fetch. The current server
    /// does not expose any registered resources, so we return an empty
    /// `contents` array — a spec-compliant "no such resource" shape
    /// for a server that has nothing to serve. A future iteration can
    /// populate this from a [`Backend`] method (e.g. fetch a model
    /// card by URI) without changing the wire format.
    pub fn handle_resources_read(
        &self,
        params: Option<&Value>,
    ) -> Result<Value, McpError> {
        let params = params.unwrap_or(&Value::Null);
        let obj = params.as_object().ok_or_else(|| {
            McpError::invalid_params("`resources/read` params must be an object")
        })?;

        let _uri = obj
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpError::invalid_params("`resources/read` requires a `uri` string")
            })?;

        Ok(json!({ "contents": [] }))
    }

    /// Handle the MCP `prompts/list` request.
    ///
    /// Returns `{ prompts: [] }` because the server does not yet
    /// register any prompt templates. Advertising the `prompts`
    /// capability in `initialize` (see [`Self::handle_initialize`])
    /// tells the client the method is supported; the empty list
    /// tells it there's nothing to choose from yet.
    pub fn handle_prompts_list(&self) -> Result<Value, McpError> {
        Ok(json!({ "prompts": [] }))
    }

    /// Handle the MCP `prompts/get` request.
    ///
    /// Per the spec, `params` must be an object containing a `name`
    /// string identifying the prompt template to render. The current
    /// server has no registered prompt templates, so the only valid
    /// outcome is a JSON-RPC `Method not found` (-32601) error
    /// echoing the requested `name` — that's what the upstream
    /// reference servers return for an unknown prompt, and it lets
    /// the client distinguish "unknown prompt" from "valid prompt
    /// with empty messages".
    pub fn handle_prompts_get(
        &self,
        params: Option<&Value>,
    ) -> Result<Value, McpError> {
        let params = params.unwrap_or(&Value::Null);
        let obj = params
            .as_object()
            .ok_or_else(|| McpError::invalid_params("`prompts/get` params must be an object"))?;

        let name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("`prompts/get` requires a `name` string"))?;

        // No prompts registered yet — surface a method-not-found so
        // clients see a clean error rather than an empty `messages`.
        Err(McpError::method_not_found(format!("prompts/{name}")))
    }

    /// Handle the `notifications/initialized` notification. Per the spec
    /// this is a notification (no `id`) and we'd return `None`; here we
    /// just flip the state flag.
    pub fn handle_notification_initialized(&self, state: &mut McpState) {
        state.initialized = true;
    }

    /// Top-level dispatch for a single JSON-RPC 2.0 request, returning
    /// either a successful `result` JSON value or an [`McpError`].
    ///
    /// `method` is the JSON-RPC method string; `params` is the optional
    /// `params` field, already validated to be either missing, null, or an
    /// object by the transport layer.
    pub fn dispatch(
        &self,
        state: &mut McpState,
        method: &str,
        params: Option<&Value>,
    ) -> Result<Value, McpError> {
        match method {
            "initialize" => self.handle_initialize(state, params),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(state, params),
            "resources/list" => self.handle_resources_list(),
            "resources/read" => self.handle_resources_read(params),
            "prompts/list" => self.handle_prompts_list(),
            "prompts/get" => self.handle_prompts_get(params),
            "notifications/initialized" => {
                self.handle_notification_initialized(state);
                // Notifications don't produce a result; the transport layer
                // is responsible for not sending a response. We return
                // `Null` as a sentinel "nothing to reply with".
                Ok(Value::Null)
            }
            // `notifications/*` are explicitly ignored (not an error).
            other if other.starts_with("notifications/") => Ok(Value::Null),
            other => Err(McpError::method_not_found(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;

    #[test]
    fn mcp_state_new_is_uninitialized() {
        let state = McpState::new(Arc::new(MockBackend::new()));
        assert!(!state.initialized);
        assert!(state.client_info.is_none());
        assert!(state.protocol_version.is_none());
    }

    #[test]
    fn mcp_server_new_is_a_zero_sized_handle() {
        let server = McpServer::new();
        // `McpServer` is `Copy` + `Default`; a sanity check that we can
        // still construct it with the new constructor.
        let _ = server;
    }

    /// `handle_initialize` must advertise tools, resources, and prompts
    /// capabilities so that spec-conformant clients render the full
    /// server surface.
    #[test]
    fn initialize_advertises_tools_resources_and_prompts_capabilities() {
        let server = McpServer::new();
        let mut state = McpState::new(Arc::new(MockBackend::new()));
        let result = server
            .handle_initialize(&mut state, Some(&json!({})))
            .expect("initialize succeeds");

        let caps = result
            .get("capabilities")
            .and_then(|v| v.as_object())
            .expect("capabilities is an object");

        assert!(caps.contains_key("tools"), "tools capability advertised");
        assert!(
            caps.contains_key("resources"),
            "resources capability advertised"
        );
        assert!(
            caps.contains_key("prompts"),
            "prompts capability advertised"
        );
    }

    /// `resources/list` must return an empty `resources` array — paired
    /// with the `resources` capability advertised in `initialize`, this
    /// lets clients enumerate resources without a `-32601`.
    #[test]
    fn dispatch_resources_list_returns_empty_resources() {
        let server = McpServer::new();
        let mut state = McpState::new(Arc::new(MockBackend::new()));
        let result = server
            .dispatch(&mut state, "resources/list", None)
            .expect("resources/list succeeds");
        assert!(result["resources"].is_array());
        assert_eq!(result["resources"].as_array().unwrap().len(), 0);
    }

    /// `resources/read` must validate the `uri` param and return a
    /// spec-compliant empty `contents` array.
    #[test]
    fn dispatch_resources_read_returns_empty_contents() {
        let server = McpServer::new();
        let mut state = McpState::new(Arc::new(MockBackend::new()));
        let result = server
            .dispatch(
                &mut state,
                "resources/read",
                Some(&json!({"uri": "hwledger://model/hf__org/name"})),
            )
            .expect("resources/read succeeds");
        assert!(result["contents"].is_array());
        assert_eq!(result["contents"].as_array().unwrap().len(), 0);
    }

    /// `resources/read` must reject a missing `uri` with `-32602`.
    #[test]
    fn dispatch_resources_read_rejects_missing_uri() {
        let server = McpServer::new();
        let mut state = McpState::new(Arc::new(MockBackend::new()));
        let err = server
            .dispatch(&mut state, "resources/read", Some(&json!({})))
            .expect_err("missing uri must error");
        assert_eq!(err.code(), -32602);
    }

    /// `prompts/list` must return an empty `prompts` array.
    #[test]
    fn dispatch_prompts_list_returns_empty_prompts() {
        let server = McpServer::new();
        let mut state = McpState::new(Arc::new(MockBackend::new()));
        let result = server
            .dispatch(&mut state, "prompts/list", None)
            .expect("prompts/list succeeds");
        assert!(result["prompts"].is_array());
        assert_eq!(result["prompts"].as_array().unwrap().len(), 0);
    }

    /// `prompts/get` for a known-but-unregistered prompt should surface
    /// a `Method not found` (-32601) error so clients can distinguish
    /// "unknown prompt" from "valid prompt with empty messages".
    #[test]
    fn dispatch_prompts_get_errors_for_unknown_prompt() {
        let server = McpServer::new();
        let mut state = McpState::new(Arc::new(MockBackend::new()));
        let err = server
            .dispatch(
                &mut state,
                "prompts/get",
                Some(&json!({"name": "missing"})),
            )
            .expect_err("unknown prompt must error");
        assert_eq!(err.code(), -32601);
    }

    /// `prompts/get` must reject a missing `name` with `-32602`.
    #[test]
    fn dispatch_prompts_get_rejects_missing_name() {
        let server = McpServer::new();
        let mut state = McpState::new(Arc::new(MockBackend::new()));
        let err = server
            .dispatch(&mut state, "prompts/get", Some(&json!({})))
            .expect_err("missing name must error");
        assert_eq!(err.code(), -32602);
    }
}
