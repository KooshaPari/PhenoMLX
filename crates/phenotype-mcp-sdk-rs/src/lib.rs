// SPDX-License-Identifier: MIT OR Apache-2.0
//! Rust MCP SDK — server trait, tool/resource abstractions, and transports.
//!
//! Provides a small, pluggable MCP server surface:
//! - [`McpServer`] trait with `list_tools`, `call_tool`, `list_resources`, `read_resource`
//! - [`Tool`] struct with a JSON schema and a boxed handler
//! - [`StdioTransport`] and [`SseTransport`] for communication
//! - [`mcp_tool!`] and [`mcp_resource!`] declarative macros for quick registration
//!
//! Traceability: audit rec #25 (Rust MCP SDK).

pub mod macros;
pub mod tool;
pub mod transport;

use async_trait::async_trait;
use serde_json::Value;

/// Core MCP server trait.
///
/// Implementors provide a catalog of tools and resources, and handle
/// invocation / read requests.  The trait is object-safe so it can be
/// boxed and used across dynamic dispatch boundaries.
#[async_trait]
pub trait McpServer: Send + Sync {
    /// List all tools exposed by this server.
    async fn list_tools(&self) -> Vec<tool::Tool>;

    /// Call a tool by name with a JSON payload.
    async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, McpError>;

    /// List all resources exposed by this server.
    async fn list_resources(&self) -> Vec<Resource>;

    /// Read a resource by its URI.
    async fn read_resource(&self, uri: &str) -> Result<ResourceContent, McpError>;
}

/// A resource exposed by an MCP server.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    pub mime_type: Option<String>,
    pub description: Option<String>,
}

/// Content of a resource read operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceContent {
    pub uri: String,
    pub mime_type: Option<String>,
    pub text: Option<String>,
    pub blob: Option<Vec<u8>>,
}

/// MCP SDK errors.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),
    #[error("resource not found: {0}")]
    ResourceNotFound(String),
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("internal error: {0}")]
    Internal(String),
}

/// Server capability descriptor.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ServerCapabilities {
    pub tools: bool,
    pub resources: bool,
    pub prompts: bool,
}

/// Server info returned during initialization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub capabilities: ServerCapabilities,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_error_display() {
        let e = McpError::ToolNotFound("foo".into());
        assert!(e.to_string().contains("foo"));
    }

    #[test]
    fn server_info_round_trip() {
        let info = ServerInfo {
            name: "test-server".into(),
            version: "0.1.0".into(),
            capabilities: ServerCapabilities {
                tools: true,
                resources: false,
                prompts: true,
            },
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: ServerInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test-server");
        assert!(back.capabilities.tools);
        assert!(!back.capabilities.resources);
    }

    #[test]
    fn resource_content_has_text_or_blob() {
        let rc = ResourceContent {
            uri: "file:///tmp/test.txt".into(),
            mime_type: Some("text/plain".into()),
            text: Some("hello".into()),
            blob: None,
        };
        assert_eq!(rc.text.unwrap(), "hello");
    }

    #[test]
    fn resource_serde_round_trip() {
        let r = Resource {
            uri: "res://docs".into(),
            name: "Documentation".into(),
            mime_type: Some("text/markdown".into()),
            description: Some("API docs".into()),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: Resource = serde_json::from_str(&json).unwrap();
        assert_eq!(back.uri, "res://docs");
    }

    #[test]
    fn mcp_error_tool_not_found() {
        let e = McpError::ToolNotFound("missing".into());
        assert!(matches!(e, McpError::ToolNotFound(_)));
    }

    #[test]
    fn mcp_error_resource_not_found() {
        let e = McpError::ResourceNotFound("missing".into());
        assert!(matches!(e, McpError::ResourceNotFound(_)));
    }

    #[test]
    fn mcp_error_invalid_arguments() {
        let e = McpError::InvalidArguments("bad json".into());
        assert!(matches!(e, McpError::InvalidArguments(_)));
    }

    #[test]
    fn mcp_error_transport() {
        let e = McpError::Transport("io broken".into());
        assert!(matches!(e, McpError::Transport(_)));
    }

    #[test]
    fn mcp_error_internal() {
        let e = McpError::Internal("panic".into());
        assert!(matches!(e, McpError::Internal(_)));
    }

    #[test]
    fn server_capabilities_default() {
        let cap = ServerCapabilities::default();
        assert!(!cap.tools);
        assert!(!cap.resources);
        assert!(!cap.prompts);
    }
}
