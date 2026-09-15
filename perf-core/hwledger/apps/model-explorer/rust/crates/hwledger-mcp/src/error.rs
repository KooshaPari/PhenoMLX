//! Crate-wide error types for the `hwledger-mcp` JSON-RPC server.
//!
//! Variants are designed to map 1:1 onto the JSON-RPC 2.0 error codes defined
//! by the spec (also reused by MCP 2024-11-05), so [`McpError::code`] always
//! returns the canonical integer the client is expecting.
//!
//! Reference: <https://www.jsonrpc.org/specification#error_object>

use thiserror::Error;

/// Errors that the MCP server can surface back to a connected MCP client.
///
/// Each variant maps to a JSON-RPC 2.0 standard error code (see
/// [`McpError::code`]).
#[derive(Debug, Error)]
pub enum McpError {
    /// The inbound frame was not valid JSON (`-32700`).
    #[error("parse error: {0}")]
    Parse(String),

    /// The JSON parsed but the envelope was not a well-formed JSON-RPC 2.0
    /// request (missing `jsonrpc`, `method`, wrong type, etc.) — `-32600`.
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// The requested method is not implemented by the server, or the named
    /// tool does not exist — `-32601`.
    #[error("method not found: {0}")]
    MethodNotFound(String),

    /// The method exists but the supplied parameters failed validation —
    /// `-32602`.
    #[error("invalid params: {0}")]
    InvalidParams(String),

    /// The server failed to honour a well-formed request — `-32603`.
    #[error("internal error: {0}")]
    Internal(String),
}

impl McpError {
    /// Return the canonical JSON-RPC 2.0 integer code for this error.
    pub fn code(&self) -> i32 {
        match self {
            McpError::Parse(_) => -32700,
            McpError::InvalidRequest(_) => -32600,
            McpError::MethodNotFound(_) => -32601,
            McpError::InvalidParams(_) => -32602,
            McpError::Internal(_) => -32603,
        }
    }

    /// Return the short, stable message string the spec recommends pairing
    /// with the integer code (e.g. `"Method not found"`).
    pub fn message(&self) -> &'static str {
        match self {
            McpError::Parse(_) => "Parse error",
            McpError::InvalidRequest(_) => "Invalid Request",
            McpError::MethodNotFound(_) => "Method not found",
            McpError::InvalidParams(_) => "Invalid params",
            McpError::Internal(_) => "Internal error",
        }
    }

    /// Convenience constructor for [`McpError::Parse`].
    pub fn parse<S: Into<String>>(msg: S) -> Self {
        Self::Parse(msg.into())
    }

    /// Convenience constructor for [`McpError::InvalidRequest`].
    pub fn invalid_request<S: Into<String>>(msg: S) -> Self {
        Self::InvalidRequest(msg.into())
    }

    /// Convenience constructor for [`McpError::MethodNotFound`].
    pub fn method_not_found<S: Into<String>>(msg: S) -> Self {
        Self::MethodNotFound(msg.into())
    }

    /// Convenience constructor for [`McpError::InvalidParams`].
    pub fn invalid_params<S: Into<String>>(msg: S) -> Self {
        Self::InvalidParams(msg.into())
    }

    /// Convenience constructor for [`McpError::Internal`].
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::Internal(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_json_rpc_spec() {
        assert_eq!(McpError::parse("x").code(), -32700);
        assert_eq!(McpError::invalid_request("x").code(), -32600);
        assert_eq!(McpError::method_not_found("x").code(), -32601);
        assert_eq!(McpError::invalid_params("x").code(), -32602);
        assert_eq!(McpError::internal("x").code(), -32603);
    }

    #[test]
    fn messages_are_stable_strings() {
        assert_eq!(McpError::parse("x").message(), "Parse error");
        assert_eq!(McpError::invalid_request("x").message(), "Invalid Request");
        assert_eq!(
            McpError::method_not_found("x").message(),
            "Method not found"
        );
        assert_eq!(McpError::invalid_params("x").message(), "Invalid params");
        assert_eq!(McpError::internal("x").message(), "Internal error");
    }
}
