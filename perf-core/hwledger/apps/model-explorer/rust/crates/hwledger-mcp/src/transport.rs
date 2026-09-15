//! Stdio transport for the MCP server.
//!
//! The transport is intentionally minimal: it reads one JSON object per
//! newline-delimited line from any [`std::io::BufRead`], dispatches it
//! through [`McpServer::dispatch`], and writes the response (if any) to
//! the supplied [`std::io::Write`].
//!
//! Two entry points matter:
//!
//! * [`handle_message`]
//!   — pure, side-effect-free parser + dispatcher. It takes a single
//!   raw JSON string and returns the raw JSON string the server should
//!   write back (or `None` for notifications). All tests target this
//!   function in isolation.
//!
//! * [`run_stdio`]
//!   — the binary's main loop: reads from stdin, writes to stdout, and
//!   exits cleanly on EOF.
//!
//! The transport is deliberately *not* async; stdio framing is line-based
//! and the work per message is tiny, so a synchronous loop is the
//! simplest correct thing.

use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

use crate::error::McpError;
use crate::{McpServer, McpState};

/// Handle a single JSON-RPC 2.0 message and return the response payload
/// the transport should write to the client.
///
/// * `input` is the raw JSON string (one full message) read from the
///   transport.
/// * `state` is the per-connection mutable state, mutated by the
///   `initialize` / `notifications/initialized` handshake.
/// * Returns `Ok(Some(response_json))` for a normal request, including
///   malformed frames (which become a JSON-RPC error response with
///   `id: null`), and `Ok(None)` for a notification (no response should
///   be sent).
///
/// Framing errors are *never* returned as `Err` — they are always
/// converted into a JSON-RPC error response so the client can observe
/// what went wrong. This matches the JSON-RPC 2.0 spec's "best-effort
/// error reporting" requirement.
pub fn handle_message(
    server: &McpServer,
    state: &mut McpState,
    input: &str,
) -> Result<Option<Value>, McpError> {
    // Step 1: parse. Anything not parseable is a JSON-RPC `-32700`.
    let request: Value = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(e) => {
            return Ok(Some(framing_error_response(
                e.to_string(),
                McpError::parse(e.to_string()),
            )));
        }
    };

    // Step 2: validate the JSON-RPC 2.0 envelope. If `id` is absent we
    // treat the message as a notification and skip the response.
    let obj = match request.as_object() {
        Some(o) => o,
        None => {
            return Ok(Some(framing_error_response(
                "frame must be a JSON object".to_string(),
                McpError::invalid_request("frame must be a JSON object"),
            )));
        }
    };

    // `jsonrpc` field must be present and equal to "2.0".
    if obj.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        return Ok(Some(framing_error_response(
            "`jsonrpc` must be \"2.0\"".to_string(),
            McpError::invalid_request("`jsonrpc` must be \"2.0\""),
        )));
    }

    let method = match obj.get("method").and_then(|v| v.as_str()) {
        Some(m) => m.to_string(),
        None => {
            return Ok(Some(framing_error_response(
                "missing `method` field".to_string(),
                McpError::invalid_request("missing `method` field"),
            )));
        }
    };

    let raw_id = obj.get("id");
    let is_notification = raw_id.is_none();

    // `params` is optional. If present, it must be an object (we don't
    // support positional arrays).
    let params = match obj.get("params") {
        None | Some(Value::Null) => None,
        Some(p) if p.is_object() => Some(p),
        Some(_) => {
            return Ok(Some(framing_error_response(
                "`params` must be an object or omitted".to_string(),
                McpError::invalid_request("`params` must be an object or omitted"),
            )));
        }
    };

    // Step 3: dispatch.
    let result = server.dispatch(state, &method, params);

    // Step 4: notifications produce no response.
    if is_notification {
        return Ok(None);
    }

    // `id` is required to be a string, number, or null. Anything else is
    // an invalid request.
    let id = match raw_id {
        Some(Value::String(_)) | Some(Value::Number(_)) | Some(Value::Null) => raw_id.unwrap(),
        Some(_) => {
            return Ok(Some(framing_error_response(
                "`id` must be a string, number, or null".to_string(),
                McpError::invalid_request("`id` must be a string, number, or null"),
            )));
        }
        None => unreachable!("checked above"),
    };

    let response = match result {
        Ok(value) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": value,
        }),
        Err(err) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": err.code(),
                "message": err.message(),
                "data": err.to_string(),
            }
        }),
    };

    Ok(Some(response))
}

/// Build a JSON-RPC 2.0 error response with `id: null`. Used for frames
/// that fail to parse or otherwise can't carry a valid `id` back.
fn framing_error_response(_detail: String, err: McpError) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": Value::Null,
        "error": {
            "code": err.code(),
            "message": err.message(),
            "data": err.to_string(),
        }
    })
}

/// Run the stdio main loop until EOF.
///
/// Reads newline-delimited JSON-RPC 2.0 requests from `reader`, writes
/// responses (one per line) to `writer`, and returns `Ok(())` on a clean
/// EOF. Any frame that fails to parse still produces a JSON-RPC error
/// response — the loop never panics and never exits early on bad input.
pub fn run_stdio<R: BufRead, W: Write>(
    server: &McpServer,
    state: &mut McpState,
    reader: R,
    writer: &mut W,
) -> io::Result<()> {
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                // A read error is a transport-level failure — bubble up.
                return Err(e);
            }
        };

        // Tolerate blank lines (common when humans hand-drive stdio).
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match handle_message(server, state, trimmed) {
            Ok(Some(response)) => {
                let serialized = serde_json::to_string(&response)
                    .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
                writeln!(writer, "{serialized}")?;
                writer.flush()?;
            }
            Ok(None) => {
                // Notification — nothing to write back.
            }
            Err(err) => {
                // Per JSON-RPC 2.0, parse errors and invalid requests get a
                // best-effort response with `id: null` so the client can
                // still observe the failure.
                let err_response = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {
                        "code": err.code(),
                        "message": err.message(),
                        "data": err.to_string(),
                    }
                });
                let serialized = serde_json::to_string(&err_response)
                    .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
                writeln!(writer, "{serialized}")?;
                writer.flush()?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MockBackend;
    use std::sync::Arc;

    fn fresh() -> (McpServer, McpState) {
        let backend: Arc<dyn crate::backend::Backend> = Arc::new(MockBackend::new());
        (McpServer::new(), McpState::new(backend))
    }

    fn as_str(val: &Value) -> String {
        serde_json::to_string(val).unwrap()
    }

    #[test]
    fn initialize_handshake() {
        let (server, mut state) = fresh();
        let req = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "clientInfo": {"name": "test-client", "version": "0.0.1"},
                "capabilities": {}
            }
        }"#;
        let resp = handle_message(&server, &mut state, req)
            .expect("initialize must succeed")
            .expect("initialize must produce a response");

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        let result = &resp["result"];
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "hwledger-mcp");
        assert_eq!(result["capabilities"]["tools"]["listChanged"], false);

        // State should have been updated.
        assert_eq!(state.protocol_version.as_deref(), Some("2024-11-05"));
        assert_eq!(state.client_info.as_ref().unwrap()["name"], "test-client");
    }

    #[test]
    fn tools_list_advertises_six_tools() {
        let (server, mut state) = fresh();
        let req = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;
        let resp = handle_message(&server, &mut state, req)
            .expect("tools/list must succeed")
            .expect("tools/list must produce a response");

        let tools = resp["result"]["tools"].as_array().expect("tools is array");
        assert_eq!(tools.len(), 6, "expected 6 tools, got {}", tools.len());

        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
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

    #[test]
    fn tools_call_success() {
        let (server, mut state) = fresh();
        let req = r#"{
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "model_search", "arguments": {"query": "tiny llm", "limit": 5}}
        }"#;
        let resp = handle_message(&server, &mut state, req)
            .expect("tools/call must succeed")
            .expect("tools/call must produce a response");

        assert_eq!(resp["id"], 3);
        let result = &resp["result"];
        assert_eq!(result["isError"], false);
        let content = result["content"].as_array().expect("content is array");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");

        // The text payload is a JSON-encoded value; it must round-trip
        // and carry the request's `query` / `limit` fields back.
        let inner: Value =
            serde_json::from_str(content[0]["text"].as_str().unwrap()).expect("text re-parses");
        assert_eq!(inner["query"], "tiny llm");
        assert_eq!(inner["limit"], 5);
        assert!(inner["results"].is_array());
    }

    #[test]
    fn unknown_tool_returns_method_not_found() {
        let (server, mut state) = fresh();
        let req = r#"{
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {"name": "no_such_tool", "arguments": {}}
        }"#;
        let resp = handle_message(&server, &mut state, req)
            .expect("transport never returns Err for a well-formed envelope")
            .expect("must produce a response");

        assert_eq!(resp["id"], 4);
        assert_eq!(resp["error"]["code"], -32601);
        assert_eq!(resp["error"]["message"], "Method not found");
    }

    #[test]
    fn invalid_tool_args_returns_invalid_params() {
        let (server, mut state) = fresh();
        // `model_detail` requires both `source` and `id`; we omit `id`.
        let req = r#"{
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {"name": "model_detail", "arguments": {"source": "hf"}}
        }"#;
        let resp = handle_message(&server, &mut state, req)
            .expect("transport never returns Err for a well-formed envelope")
            .expect("must produce a response");

        assert_eq!(resp["id"], 5);
        assert_eq!(resp["error"]["code"], -32602);
        assert_eq!(resp["error"]["message"], "Invalid params");
    }

    #[test]
    fn notification_produces_no_response() {
        let (server, mut state) = fresh();
        let req = r#"{
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }"#;
        let resp = handle_message(&server, &mut state, req).expect("notifications must not raise");
        assert!(resp.is_none(), "notifications must produce no response");
        assert!(state.initialized, "notification must flip state");

        // And the missing-`id` variant of every other notification, too.
        let req2 =
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"reason":"user"}}"#;
        let resp2 = handle_message(&server, &mut state, req2).expect("must not raise");
        assert!(resp2.is_none());
    }

    #[test]
    fn multi_message_roundtrip() {
        let (server, mut state) = fresh();

        // Stage input as a single newline-delimited stream so we exercise
        // the same framing `run_stdio` would see.
        let input = "\
{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\"}}
{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}
{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"model_search\",\"arguments\":{\"query\":\"q\"}}}
{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}
{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"model_detail\",\"arguments\":{\"source\":\"hf\",\"id\":\"o/m\"}}}
";

        let mut outputs: Vec<String> = Vec::new();
        for line in input.lines() {
            if let Some(resp) = handle_message(&server, &mut state, line).unwrap() {
                outputs.push(as_str(&resp));
            }
        }

        // 4 responses (the notification must not produce one).
        assert_eq!(outputs.len(), 4, "got outputs: {outputs:#?}");

        let parsed: Vec<Value> = outputs
            .iter()
            .map(|s| serde_json::from_str(s).expect("response is JSON"))
            .collect();

        assert_eq!(parsed[0]["id"], 1);
        assert_eq!(parsed[0]["result"]["protocolVersion"], "2024-11-05");

        assert_eq!(parsed[1]["id"], 2);
        assert_eq!(parsed[1]["result"]["tools"].as_array().unwrap().len(), 6);

        assert_eq!(parsed[2]["id"], 3);
        assert_eq!(parsed[2]["result"]["isError"], false);

        assert_eq!(parsed[3]["id"], 4);
        assert_eq!(parsed[3]["result"]["isError"], false);

        // State flag was flipped by the notification.
        assert!(state.initialized);
    }

    #[test]
    fn parse_error_returns_negative_32700() {
        let (server, mut state) = fresh();
        let resp = handle_message(&server, &mut state, "{not even json")
            .expect("transport never returns Err for a well-formed envelope")
            .expect("parse error still produces a response");

        assert_eq!(resp["id"], Value::Null);
        assert_eq!(resp["error"]["code"], -32700);
        assert_eq!(resp["error"]["message"], "Parse error");
    }
}
