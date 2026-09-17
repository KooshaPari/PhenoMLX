// SPDX-License-Identifier: MIT OR Apache-2.0
//! MCP transports — stdio and SSE.
//!
//! Both transports are async and work with the [`McpServer`] trait.
//! [`StdioTransport`] reads newline-delimited JSON from stdin and writes
//! responses to stdout.  [`SseTransport`] is a placeholder for HTTP
//! Server-Sent Events streaming.

use crate::{McpError, McpServer};
use serde_json::Value;
use std::io::{self, BufRead, Write};
use tokio::sync::Mutex;

/// Stdio transport for MCP JSON-RPC messages.
///
/// Reads one JSON object per line from stdin, dispatches to the server,
/// and writes the response as a single JSON line to stdout.
pub struct StdioTransport<S: McpServer> {
    server: Mutex<S>,
}

impl<S: McpServer> StdioTransport<S> {
    /// Wrap the given server.
    pub fn new(server: S) -> Self {
        Self {
            server: Mutex::new(server),
        }
    }

    /// Run the transport loop on the current thread.
    ///
    /// This is a blocking call that processes stdin until EOF.
    pub async fn run(&self) -> Result<(), McpError> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        for line in stdin.lock().lines() {
            let line = line.map_err(|e| McpError::Transport(e.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let req: serde_json::Value = serde_json::from_str(&line)
                .map_err(|e| McpError::InvalidArguments(e.to_string()))?;
            let resp = self.handle_request(req).await?;
            let out =
                serde_json::to_string(&resp).map_err(|e| McpError::Internal(e.to_string()))?;
            writeln!(stdout, "{}", out).map_err(|e| McpError::Transport(e.to_string()))?;
            stdout
                .flush()
                .map_err(|e| McpError::Transport(e.to_string()))?;
        }
        Ok(())
    }

    async fn handle_request(&self, req: Value) -> Result<Value, McpError> {
        let method = req
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown");
        let server = self.server.lock().await;
        match method {
            "tools/list" => {
                let tools = server.list_tools().await;
                let tools_json: Vec<serde_json::Value> = tools
                    .into_iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "description": t.description,
                            "inputSchema": t.input_schema,
                        })
                    })
                    .collect();
                Ok(serde_json::json!({ "tools": tools_json }))
            }
            "tools/call" => {
                let name = req
                    .get("params")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .ok_or_else(|| McpError::InvalidArguments("missing name".into()))?;
                let args = req
                    .get("params")
                    .and_then(|p| p.get("arguments"))
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                let result = server.call_tool(name, args).await?;
                Ok(serde_json::json!({ "result": result }))
            }
            "resources/list" => {
                let resources = server.list_resources().await;
                Ok(serde_json::json!({ "resources": resources }))
            }
            "resources/read" => {
                let uri = req
                    .get("params")
                    .and_then(|p| p.get("uri"))
                    .and_then(|u| u.as_str())
                    .ok_or_else(|| McpError::InvalidArguments("missing uri".into()))?;
                let content = server.read_resource(uri).await?;
                Ok(serde_json::json!({ "content": content }))
            }
            _ => Err(McpError::InvalidArguments(format!(
                "unknown method: {}",
                method
            ))),
        }
    }
}

impl<S: McpServer> std::fmt::Debug for StdioTransport<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdioTransport").finish_non_exhaustive()
    }
}

/// SSE transport placeholder.
///
/// Will bind to an HTTP endpoint and stream MCP messages over
/// Server-Sent Events.  Currently this is a stub; the full
/// implementation requires an HTTP server framework.
pub struct SseTransport<S: McpServer> {
    server: Mutex<S>,
}

impl<S: McpServer> SseTransport<S> {
    /// Wrap the given server.
    pub fn new(server: S) -> Self {
        Self {
            server: Mutex::new(server),
        }
    }

    /// Placeholder for the SSE serve loop.
    pub async fn serve(&self, _addr: &str) -> Result<(), McpError> {
        // TODO: implement Axum/Actix-based SSE streaming.
        Ok(())
    }

    /// Access the inner server.
    pub async fn server(&self) -> tokio::sync::MutexGuard<'_, S> {
        self.server.lock().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::Tool;
    use async_trait::async_trait;
    use serde_json::json;

    struct MockServer;

    #[async_trait]
    impl McpServer for MockServer {
        async fn list_tools(&self) -> Vec<Tool> {
            vec![Tool::new("echo", "echo", json!({}), |args: Value| Ok(args))]
        }
        async fn call_tool(&self, _name: &str, _arguments: Value) -> Result<Value, McpError> {
            Ok(json!("ok"))
        }
        async fn list_resources(&self) -> Vec<crate::Resource> {
            vec![]
        }
        async fn read_resource(&self, _uri: &str) -> Result<crate::ResourceContent, McpError> {
            Err(McpError::ResourceNotFound("x".into()))
        }
    }

    #[tokio::test]
    async fn stdio_transport_new() {
        let transport = StdioTransport::new(MockServer);
        assert!(transport.server.lock().await.list_tools().await.len() == 1);
    }

    #[tokio::test]
    async fn sse_transport_new() {
        let transport = SseTransport::new(MockServer);
        assert!(transport.serve("127.0.0.1:0").await.is_ok());
    }

    #[tokio::test]
    async fn sse_transport_server_access() {
        let transport = SseTransport::new(MockServer);
        let tools = transport.server().await.list_tools().await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
    }

    #[test]
    fn stdio_transport_debug() {
        let t = StdioTransport::new(MockServer);
        // Just verify it doesn't panic
        let _ = format!("{:?}", t);
    }

    #[tokio::test]
    async fn stdio_transport_handle_tools_list() {
        let transport = StdioTransport::new(MockServer);
        let req = json!({"method": "tools/list"});
        let resp = transport.handle_request(req).await.unwrap();
        assert!(resp.get("tools").is_some());
    }

    #[tokio::test]
    async fn stdio_transport_handle_unknown_method() {
        let transport = StdioTransport::new(MockServer);
        let req = json!({"method": "bogus"});
        let resp = transport.handle_request(req).await;
        assert!(resp.is_err());
        assert!(resp.unwrap_err().to_string().contains("unknown method"));
    }

    #[tokio::test]
    async fn stdio_transport_handle_tools_call() {
        let transport = StdioTransport::new(MockServer);
        let req = json!({"method": "tools/call", "params": {"name": "echo", "arguments": {}}});
        let resp = transport.handle_request(req).await.unwrap();
        assert!(resp.get("result").is_some());
    }
}
