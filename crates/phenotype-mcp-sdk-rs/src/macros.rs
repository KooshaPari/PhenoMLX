// SPDX-License-Identifier: MIT OR Apache-2.0
//! Declarative macros for the MCP SDK.
//!
//! [`mcp_tool!`] and [`mcp_resource!`] reduce boilerplate when registering
//! tools and resources in a server implementation.

/// Build a [`Tool`](crate::tool::Tool) with a JSON schema and a handler.
///
/// # Example
///
/// ```
/// use phenotype_mcp_sdk_rs::{mcp_tool, tool::Tool};
/// use serde_json::json;
///
/// let t = mcp_tool!("echo", "Echo the input", {"type": "object"}, |args| Ok(args));
/// assert_eq!(t.name, "echo");
/// ```
#[macro_export]
macro_rules! mcp_tool {
    ($name:expr, $desc:expr, $schema:tt, $handler:expr) => {{
        $crate::tool::Tool::new($name, $desc, serde_json::json!($schema), $handler)
    }};
}

/// Build a [`Resource`](crate::Resource) struct.
///
/// # Example
///
/// ```
/// use phenotype_mcp_sdk_rs::{mcp_resource, Resource};
///
/// let r = mcp_resource!("res://docs", "Documentation", "text/markdown", "API docs");
/// assert_eq!(r.uri, "res://docs");
/// ```
#[macro_export]
macro_rules! mcp_resource {
    ($uri:expr, $name:expr, $mime:expr, $desc:expr) => {{
        $crate::Resource {
            uri: ($uri).into(),
            name: ($name).into(),
            mime_type: Some(($mime).into()),
            description: Some(($desc).into()),
        }
    }};
    ($uri:expr, $name:expr) => {{
        $crate::Resource {
            uri: ($uri).into(),
            name: ($name).into(),
            mime_type: None,
            description: None,
        }
    }};
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn mcp_tool_macro_builds_tool() {
        let t = mcp_tool!("add", "Add two numbers", {"type": "object"}, |args: serde_json::Value| {
            let a = args["a"].as_i64().unwrap_or(0);
            let b = args["b"].as_i64().unwrap_or(0);
            Ok(serde_json::json!(a + b))
        });
        assert_eq!(t.name, "add");
        assert_eq!(t.description, "Add two numbers");
        let result = t.call(json!({"a": 2, "b": 3})).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn mcp_resource_macro_full() {
        let r = mcp_resource!("res://docs", "Documentation", "text/markdown", "API docs");
        assert_eq!(r.uri, "res://docs");
        assert_eq!(r.name, "Documentation");
        assert_eq!(r.mime_type, Some("text/markdown".into()));
        assert_eq!(r.description, Some("API docs".into()));
    }

    #[test]
    fn mcp_resource_macro_short() {
        let r = mcp_resource!("res://readme", "Readme");
        assert_eq!(r.uri, "res://readme");
        assert_eq!(r.name, "Readme");
        assert!(r.mime_type.is_none());
        assert!(r.description.is_none());
    }

    #[test]
    fn mcp_tool_macro_error_case() {
        let t = mcp_tool!("fail", "Always fails", {}, |_args: serde_json::Value| {
            Err("error".into())
        });
        let result = t.call(json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn mcp_tool_macro_schema_preserved() {
        let t =
            mcp_tool!("greet", "Say hello", {"type": "object", "properties": {}}, |args| Ok(args));
        assert_eq!(t.input_schema, json!({"type": "object", "properties": {}}));
    }
}
