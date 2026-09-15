//! HTTP + Server-Sent-Events transport for the MCP server.
//!
//! This module exposes the same
//! [`McpServer::dispatch`](crate::McpServer::dispatch) core the stdio
//! transport uses, but wrapped in an `axum` router so an MCP client that
//! prefers HTTP (e.g. a browser, a server-to-server caller, or an LLM
//! harness that already speaks SSE) can talk to the server without
//! spawning a child process per session.
//!
//! ## Endpoints
//!
//! | Method | Path   | Purpose                                                   |
//! |--------|--------|-----------------------------------------------------------|
//! | POST   | `/mcp` | Synchronous JSON-RPC 2.0 request → JSON-RPC 2.0 response. |
//! | GET    | `/sse` | Server-Sent Events stream: emits the session endpoint URL  |
//! |        |        | on connect and keeps the connection alive for streaming   |
//! |        |        | notifications.                                            |
//!
//! Both endpoints share a single [`HttpState`] carrying an
//! [`Arc<McpServer>`] and the [`Arc<ServiceBackend>`]. Per-server
//! mutable state (the `McpState` initialized flag) lives behind a
//! `tokio::sync::Mutex` so concurrent requests are serialised through
//! the same handshake the stdio path uses.
//!
//! ## SSE shape
//!
//! On `GET /sse` the server emits at minimum a single named event
//! `endpoint` whose `data:` line is the absolute URL the client should
//! POST subsequent JSON-RPC frames to. The connection is held open
//! (`Content-Type: text/event-stream`) so future streaming
//! notifications can be pushed; for the v1 close-out we emit the
//! endpoint event plus periodic heartbeats and let clients reconnect
//! if they need to.
//!
//! ## SSE implementation note
//!
//! `axum` 0.7 does not ship a first-party `sse` helper, so we hand-roll
//! the SSE envelope by emitting UTF-8 bytes in the
//! `text/event-stream` format directly into a streaming response
//! body. The wire format is just `event:` / `data:` / `id:` lines
//! terminated by a blank line; we don't need a full SSE library to
//! produce it.
use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::backend::ServiceBackend;
use crate::{McpServer, McpState};

/// Shared HTTP transport state.
///
/// `McpServer` is `Copy + Default` (it carries no per-call data) so it's
/// fine to hold directly behind an `Arc`. The `Backend` is the real (or
/// mock) tool implementation — production code holds an
/// `Arc<ServiceBackend>` (which auto-derefs to `dyn Backend`), tests
/// hold an `Arc<MockBackend>`. `McpState` is the per-server mutable
/// record the `initialize` handshake writes into; we wrap it in a
/// `Mutex` because HTTP handlers can interleave on the same server
/// instance.
#[derive(Clone)]
pub struct HttpState {
    /// The pure dispatch core — `Copy`, but held in an `Arc` for symmetry
    /// with [`HttpState::backend`] and so test code can construct a
    /// single value once and clone it into every request.
    pub server: Arc<McpServer>,
    /// Real (or mock) tool backend. Typed as a `dyn Backend` so the
    /// same state works for production (`Arc<ServiceBackend>`) and
    /// tests (`Arc<MockBackend>`) without forcing tests to drag in a
    /// tantivy store + the `ServiceBackend`'s owned tokio runtime
    /// (which would panic on drop inside a `#[tokio::test]` runtime).
    pub backend: Arc<dyn crate::backend::Backend>,
    /// Per-server mutable state (initialised flag, client info, ...).
    /// Behind a `tokio::Mutex` because HTTP request handlers run on
    /// the multi-threaded runtime.
    pub state: Arc<Mutex<McpState>>,
}

impl HttpState {
    /// Build a fresh [`HttpState`] from an already-opened tantivy
    /// `ServiceBackend`. Convenience constructor so callers (the
    /// binary, integration tests) don't have to plumb three fields
    /// manually.
    #[must_use]
    pub fn new(backend: Arc<ServiceBackend>) -> Self {
        let backend: Arc<dyn crate::backend::Backend> = backend;
        Self {
            server: Arc::new(McpServer::new()),
            backend: backend.clone(),
            state: Arc::new(Mutex::new(McpState::new(backend))),
        }
    }

    /// Build an [`HttpState`] from any [`Backend`] implementor.
    ///
    /// Production code uses [`HttpState::new`]; tests use this
    /// constructor with an `Arc<MockBackend>` so they don't have to
    /// open a tantivy store or risk the "cannot drop a runtime
    /// inside another runtime" panic that the `ServiceBackend`'s
    /// owned runtime would otherwise hit when the `#[tokio::test]`
    /// runtime tears it down.
    #[cfg(test)]
    fn with_backend(backend: Arc<dyn crate::backend::Backend>) -> Self {
        Self {
            server: Arc::new(McpServer::new()),
            backend: backend.clone(),
            state: Arc::new(Mutex::new(McpState::new(backend))),
        }
    }
}

/// Build the axum [`Router`] for the HTTP+SSE transport.
///
/// The router is layered with:
/// * `TraceLayer::new_for_http()` — `tower-http` request/response
///   logging (matches the existing `hwledger-server` setup).
/// * `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)`
///   — CORS allow-all so browser-based MCP clients can call us without
///   a proxy. CORS preflight (`OPTIONS`) is implicitly handled by
///   `axum` via `allow_methods(Any)`.
pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/mcp", post(handle_post_mcp))
        .route("/sse", get(handle_get_sse))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
}

/// `POST /mcp` — synchronous per-request JSON-RPC 2.0.
///
/// Body: a JSON-RPC 2.0 request object. Response: a JSON-RPC 2.0
/// response object on `200 OK`. Parse / envelope errors still produce a
/// 200 with a JSON-RPC `error` payload (matching the stdio transport's
/// "best-effort error reporting" guarantee).
async fn handle_post_mcp(
    State(state): State<HttpState>,
    body: Result<Json<Value>, axum::extract::rejection::JsonRejection>,
) -> Response {
    // Step 1: extract the raw JSON value. Axum's `Json` extractor
    // rejects non-JSON bodies with a 4xx — we surface that as a
    // JSON-RPC parse error so the client still sees the standard
    // envelope shape.
    let raw = match body {
        Ok(Json(v)) => v,
        Err(rej) => {
            let err = crate::error::McpError::parse(rej.body_text());
            return json_rpc_error_response(Value::Null, err);
        }
    };

    // Step 2: serialise back to a string for `handle_message`. We
    // could refactor `handle_message` to accept a `Value` directly,
    // but going through the string keeps the two transports
    // provably identical (the stdio transport only ever has a
    // string).
    let raw_str = match serde_json::to_string(&raw) {
        Ok(s) => s,
        Err(e) => {
            let err = crate::error::McpError::internal(format!(
                "failed to serialise inbound JSON: {e}"
            ));
            return json_rpc_error_response(Value::Null, err);
        }
    };

    // Step 3: dispatch through the shared core. The stdio transport
    // does this on the calling thread; we hold the state mutex for
    // the duration of the call so two concurrent `initialize`
    // requests can't race the initialised flag.
    let server = state.server.clone();
    let result = {
        let mut guard = state.state.lock().await;
        crate::transport::handle_message(server.as_ref(), &mut guard, &raw_str)
    };

    // Step 4: shape the result into an HTTP response.
    match result {
        Ok(Some(resp)) => Json(resp).into_response(),
        Ok(None) => {
            // Notification — JSON-RPC 2.0 says we MUST NOT send a
            // response. Return `204 No Content` so an LLM client
            // can distinguish "I just sent a notification" from
            // "I got a real answer".
            StatusCode::NO_CONTENT.into_response()
        }
        Err(err) => json_rpc_error_response(Value::Null, err),
    }
}

/// `GET /sse` — Server-Sent Events stream.
///
/// On connect the server emits a single named `endpoint` event whose
/// `data:` payload is the absolute URL the client should POST JSON-RPC
/// requests to (the same `/mcp` endpoint, resolved against the
/// `Host` header so the URL works behind reverse proxies). The
/// connection then stays open — additional notifications can be
/// pushed later by yielding more events from the stream.
///
/// The response is a `text/event-stream` body whose chunks are
/// pre-formatted SSE frames (`event: ...\ndata: ...\nid: ...\n\n`).
/// We use a [`Body::from_stream`] so the framework flushes each
/// chunk as soon as the inner stream yields it.
async fn handle_get_sse(
    State(state): State<HttpState>,
    req: axum::http::Request<Body>,
) -> Response {
    // Resolve the endpoint URL the client should POST to. We use the
    // `Host` header (and the request URI's scheme — axum doesn't
    // expose the scheme directly, but `https` is the common case for
    // EventSource clients and `http` is fine for local dev).
    let endpoint_url = resolve_endpoint_url(&req);

    // Hold an `HttpState` clone inside the stream so future streaming
    // notifications can be added without changing the function
    // signature.
    let _state = state;

    let stream = async_stream::stream! {
        // 1. The required "endpoint" event — MCP / EventSource clients
        //    expect this exact name + data shape so they know where
        //    to POST subsequent requests.
        yield Ok::<_, std::io::Error>(format_sse_event(
            Some("endpoint"),
            Some(&endpoint_url),
            Some("0"),
        ));

        // 2. Periodic heartbeat events so reverse proxies / load
        //    balancers don't reap the connection. EventSource
        //    clients ignore unknown event names by default.
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(15));
        // The first tick fires immediately — skip it; we just sent
        // the endpoint event and don't want a burst of heartbeats.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            yield Ok(format_sse_event(
                Some("ping"),
                Some(&json!({"ts": now_secs()}).to_string()),
                None,
            ));
        }
    };

    let body = Body::from_stream(stream);

    // Build the response with the SSE-specific Content-Type and
    // Cache-Control so middleboxes don't buffer the stream.
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static("text/event-stream"))
        .header(header::CACHE_CONTROL, HeaderValue::from_static("no-cache, no-transform"))
        .header("x-accel-buffering", HeaderValue::from_static("no"))
        .body(body)
        .expect("static response builder cannot fail");

    // Stamp the connection header explicitly — some HTTP/2 clients
    // rely on it for streaming semantics.
    response.headers_mut().insert(
        header::CONNECTION,
        HeaderValue::from_static("keep-alive"),
    );
    response
}

/// Resolve the absolute POST URL the SSE client should send JSON-RPC
/// requests to.
///
/// We reconstruct it from the request's `Host` header so reverse
/// proxies / load balancers don't bake the listen-side bind address
/// into the URL. Scheme detection is best-effort: most EventSource
/// clients are browsers which only use SSE over HTTPS in production,
/// but local dev is plain HTTP.
fn resolve_endpoint_url<B>(req: &axum::http::Request<B>) -> String {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");

    // Heuristic: if the host header carries an explicit port (`:9000`)
    // or starts with `localhost` / `127.`, we treat the scheme as
    // `http`. Set `HWLEDGER_MCP_TLS=1` (or any value other than
    // `0`/`false`) to force https.
    let tls_forced_on = std::env::var("HWLEDGER_MCP_TLS")
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
        .unwrap_or(false);

    let scheme = if tls_forced_on {
        "https"
    } else if host.starts_with("localhost") || host.starts_with("127.") || host.contains(':') {
        "http"
    } else {
        // External host with no explicit port — assume HTTPS for
        // safety; client can override via HWLEDGER_MCP_TLS=0.
        "https"
    };

    format!("{scheme}://{host}/mcp")
}

/// Format a single SSE event frame.
///
/// Wire format (each line `\n`-terminated, blank line ends the event):
/// ```text
/// event: <event>
/// id: <id>
/// data: <data>
/// ```
/// All three fields are optional — omitting them produces a comment
/// frame, which we don't use here.
fn format_sse_event(event: Option<&str>, data: Option<&str>, id: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(ev) = event {
        out.push_str("event: ");
        out.push_str(ev);
        out.push('\n');
    }
    if let Some(i) = id {
        out.push_str("id: ");
        out.push_str(i);
        out.push('\n');
    }
    if let Some(d) = data {
        // SSE `data:` lines must use `\n` as their own separator and
        // a single trailing newline. JSON is single-line so this is
        // trivial; if we ever start streaming multi-line payloads
        // we'll need to split on `\n` here.
        out.push_str("data: ");
        out.push_str(d);
        out.push('\n');
    }
    out.push('\n');
    out
}

/// Lightweight "seconds since unix epoch" without pulling in `chrono`
/// (the SSE heartbeat only needs to be monotonically increasing, so
/// the system clock is fine).
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Build a JSON-RPC 2.0 error response with the given `id` and map it
/// into an `axum` 200 response. We always use 200 because the JSON-RPC
/// envelope *is* the error channel — using a 4xx would force every
/// client to special-case HTTP-level vs. protocol-level failures.
fn json_rpc_error_response(id: Value, err: crate::error::McpError) -> Response {
    let body = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": err.code(),
            "message": err.message(),
            "data": err.to_string(),
        }
    });
    (StatusCode::OK, Json(body)).into_response()
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::backend::{Backend, MockBackend};

    /// Build an `HttpState` whose dispatcher sees a `MockBackend`.
    ///
    /// The `HttpState::with_backend` constructor takes care of the
    /// dummy `ServiceBackend` placeholder, so the dispatcher can run
    /// entirely off the mock without ever touching tantivy or the
    /// owned runtime inside `ServiceBackend`.
    fn test_state() -> HttpState {
        let mock: Arc<dyn Backend> = Arc::new(MockBackend::new());
        HttpState::with_backend(mock)
    }

    /// Build the full axum router for in-process testing.
    fn app() -> Router {
        router(test_state())
    }

    /// POST a JSON-RPC request and return `(status, parsed JSON)`.
    async fn post_json(app: Router, req: Value) -> (StatusCode, Value) {
        let body = Body::from(serde_json::to_vec(&req).unwrap());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let parsed: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, parsed)
    }

    #[tokio::test]
    async fn post_mcp_initialize_returns_server_info() {
        let resp = post_json(
            app(),
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "clientInfo": {"name": "http-test", "version": "0.0.1"},
                    "capabilities": {}
                }
            }),
        )
        .await;

        assert_eq!(resp.0, StatusCode::OK);
        let body = resp.1;
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["id"], 1);
        let result = &body["result"];
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "hwledger-mcp");
        assert!(
            result["serverInfo"]["version"]
                .as_str()
                .map(|s| !s.is_empty())
                .unwrap_or(false),
            "serverInfo.version must be present"
        );
        assert_eq!(result["capabilities"]["tools"]["listChanged"], false);
    }

    #[tokio::test]
    async fn post_mcp_tools_list_returns_six_tools() {
        let resp = post_json(
            app(),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        )
        .await;

        assert_eq!(resp.0, StatusCode::OK);
        let tools = resp.1["result"]["tools"]
            .as_array()
            .expect("tools is an array");
        assert_eq!(tools.len(), 6, "expected 6 tools, got {}", tools.len());

        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["name"].as_str().unwrap_or(""))
            .collect();
        for expected in [
            "model_search",
            "model_detail",
            "model_rag_ask",
            "model_quants",
            "similar_models",
            "models_for_use_case",
        ] {
            assert!(
                names.contains(&expected),
                "missing tool `{expected}` in {names:?}"
            );
        }
    }

    #[tokio::test]
    async fn post_mcp_tools_call_model_search_returns_results() {
        let resp = post_json(
            app(),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "model_search",
                    "arguments": {"query": "tiny llm", "limit": 5}
                }
            }),
        )
        .await;

        assert_eq!(resp.0, StatusCode::OK);
        let body = resp.1;
        assert_eq!(body["id"], 3);
        let result = &body["result"];
        assert_eq!(result["isError"], false);
        let content = result["content"].as_array().expect("content is an array");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");

        // The MockBackend echoes `query` + `limit` back inside the text payload.
        let inner: Value =
            serde_json::from_str(content[0]["text"].as_str().unwrap()).expect("text re-parses");
        assert_eq!(inner["query"], "tiny llm");
        assert_eq!(inner["limit"], 5);
        assert!(inner["results"].is_array(), "results is an array");
        assert_eq!(inner["total"], 1, "MockBackend returns one hit");
    }

    #[tokio::test]
    async fn post_mcp_unknown_tool_returns_method_not_found_error() {
        let resp = post_json(
            app(),
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {"name": "no_such_tool", "arguments": {}}
            }),
        )
        .await;

        assert_eq!(resp.0, StatusCode::OK);
        assert_eq!(resp.1["id"], 4);
        assert_eq!(resp.1["error"]["code"], -32601);
        assert_eq!(resp.1["error"]["message"], "Method not found");
    }

    #[tokio::test]
    async fn post_mcp_parse_error_returns_negative_32700() {
        let app = app();
        let body = Body::from("{not even json");
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let parsed: Value = serde_json::from_slice(&bytes).expect("response is JSON");
        assert_eq!(parsed["error"]["code"], -32700);
        assert_eq!(parsed["error"]["message"], "Parse error");
    }

    #[tokio::test]
    async fn sse_emits_endpoint_event_with_session_url() {
        let app = app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/sse")
                    .header("host", "localhost:9000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let ctype = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ctype.starts_with("text/event-stream"),
            "Content-Type must be text/event-stream, got `{ctype}`"
        );

        // Read just enough of the stream to capture the first event.
        // The router keeps emitting heartbeats every 15s which would
        // otherwise hang the test, so we bound the read with a short
        // timeout and a hard byte cap.
        let mut body = resp.into_body();
        let mut collected = Vec::new();
        let read = tokio::time::timeout(std::time::Duration::from_millis(500), async {
            loop {
                // `axum::body::Body::data()` returns the next frame;
                // `None` means the stream finished cleanly.
                match body.frame().await {
                    Some(Ok(frame)) => {
                        if let Some(data) = frame.data_ref() {
                            collected.extend_from_slice(data);
                        }
                        if collected.windows(2).any(|w| w == b"\n\n") {
                            break;
                        }
                        if collected.len() > 4096 {
                            break;
                        }
                    }
                    Some(Err(_)) | None => break,
                }
            }
        })
        .await;
        // A timeout is fine — the stream *will* keep emitting forever
        // but we only care about the first event. We just unwrap the
        // inner result so a panic in the read loop doesn't get
        // swallowed silently.
        let _ = read;
        let text = String::from_utf8_lossy(&collected);

        // SSE wire format: `event: endpoint\ndata: <url>\nid: 0\n\n`
        assert!(
            text.contains("event: endpoint"),
            "missing `event: endpoint` line, got:\n{text}"
        );
        assert!(
            text.contains("/mcp"),
            "endpoint data must contain `/mcp`, got:\n{text}"
        );
        assert!(
            text.contains("http://localhost:9000/mcp"),
            "endpoint URL must include the resolved host, got:\n{text}"
        );
        assert!(
            text.contains("id: 0"),
            "endpoint event must carry `id: 0`, got:\n{text}"
        );
    }
}
