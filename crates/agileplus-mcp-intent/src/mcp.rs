// SPDX-License-Identifier: MIT OR Apache-2.0
//! MCP (Model Context Protocol) server over stdio.
//!
//! Implements a minimal JSON-RPC 2.0 server that exposes:
//! - `initialize`
//! - `convert_prompt_to_intent_graph`
//!
//! Protocol: https://modelcontextprotocol.io

use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::converter::convert;
use crate::types::ConvertRequest;
use crate::validator::validate_and_wrap;

/// JSON-RPC request envelope.
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // serde-deserialized field - consumed by downstream JSON-RPC dispatch
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

/// JSON-RPC response envelope.
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// Server capability info.
#[derive(Debug, Serialize)]
struct ServerInfo {
    name: String,
    version: String,
}

/// Run the MCP server loop on stdin/stdout.
pub fn run_stdio_server() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut lines = stdin.lock().lines();

    tracing::info!(name = "agileplus-mcp-intent", transport = "stdio", "MCP stdio server started");

    while let Some(Ok(line)) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {e}"),
                        data: None,
                    }),
                };
                send(&mut stdout, &resp)?;
                continue;
            }
        };

        let resp = handle_request(req);
        send(&mut stdout, &resp)?;
    }

    Ok(())
}

fn handle_request(req: JsonRpcRequest) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": ServerInfo {
                    name: "agileplus-mcp-intent".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                }
            })),
            error: None,
        },
        "tools/list" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: Some(serde_json::json!({
                "tools": [
                    {
                        "name": "convert_prompt_to_intent_graph",
                        "description": "Convert a user prompt into an AgilePlus intent graph conforming to the ontology",
                        "inputSchema": {
                            "type": "object",
                            "required": ["prompt"],
                            "properties": {
                                "prompt": {
                                    "type": "string",
                                    "description": "Natural language user prompt"
                                },
                                "options": {
                                    "type": "object",
                                    "properties": {
                                        "auto_decompose": { "type": "boolean" },
                                        "max_features": { "type": "integer", "minimum": 1, "maximum": 10 },
                                        "store": { "type": "boolean" }
                                    }
                                }
                            }
                        }
                    }
                ]
            })),
            error: None,
        },
        "tools/call" => handle_tool_call(req),
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", req.method),
                data: None,
            }),
        },
    }
}

fn handle_tool_call(req: JsonRpcRequest) -> JsonRpcResponse {
    let params = match req.params {
        Some(p) => p,
        None => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32602,
                    message: "Missing params".to_string(),
                    data: None,
                }),
            };
        }
    };

    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

    if tool_name != "convert_prompt_to_intent_graph" {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32602,
                message: format!("Unknown tool: {tool_name}"),
                data: None,
            }),
        };
    }

    let request: ConvertRequest = match serde_json::from_value(arguments) {
        Ok(r) => r,
        Err(e) => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32602,
                    message: format!("Invalid arguments: {e}"),
                    data: None,
                }),
            };
        }
    };

    match convert(&request.prompt, &request.options) {
        Ok(response) => match validate_and_wrap(response) {
            Ok(validated) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(serde_json::json!({
                    "content": [
                        {
                            "type": "text",
                            "text": serde_json::to_string_pretty(&validated.graph).unwrap_or_default()
                        }
                    ],
                    "isError": false
                })),
                error: None,
            },
            Err(err) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(serde_json::json!({
                    "content": [
                        {
                            "type": "text",
                            "text": serde_json::to_string_pretty(&err).unwrap_or_default()
                        }
                    ],
                    "isError": true
                })),
                error: None,
            },
        },
        Err(e) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32603,
                message: format!("Conversion failed: {e}"),
                data: None,
            }),
        },
    }
}

fn send(stdout: &mut io::Stdout, resp: &JsonRpcResponse) -> io::Result<()> {
    let line = serde_json::to_string(resp)?;
    writeln!(stdout, "{line}")?;
    stdout.flush()
}
