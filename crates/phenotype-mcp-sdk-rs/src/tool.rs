// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tool abstraction for the MCP SDK.
//!
//! A [`Tool`] is a named operation with a JSON schema and a boxed
//! handler function.  The handler receives a [`serde_json::Value`] and
//! returns a [`Result<serde_json::Value>`].

use serde_json::Value;
use std::fmt;

/// An MCP tool.
///
/// The handler is boxed so tools can be stored in a `Vec` or `HashMap`
/// without generics.  Clone is *not* derived because the handler is a
/// trait object; use `Tool::clone_with_handler` if you need to copy a
/// tool definition and replace the handler.
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub handler: Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>,
}

impl Tool {
    /// Create a new tool.
    pub fn new<F>(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value) -> Result<Value, String> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            handler: Box::new(handler),
        }
    }

    /// Invoke the tool handler.
    pub fn call(&self, args: Value) -> Result<Value, String> {
        (self.handler)(args)
    }

    /// Clone the tool metadata and swap in a new handler.
    pub fn clone_with_handler<F>(&self, handler: F) -> Self
    where
        F: Fn(Value) -> Result<Value, String> + Send + Sync + 'static,
    {
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
            handler: Box::new(handler),
        }
    }
}

impl fmt::Debug for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .field("handler", &"<Box<dyn Fn>>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_creation_and_call() {
        let t = Tool::new(
            "echo",
            "Echo the input back",
            json!({"type": "object", "properties": {}}),
            |args: Value| Ok(args),
        );
        assert_eq!(t.name, "echo");
        let result = t.call(json!({"msg": "hello"})).unwrap();
        assert_eq!(result, json!({"msg": "hello"}));
    }

    #[test]
    fn tool_call_error_propagation() {
        let t = Tool::new("fail", "Always fails", json!({}), |_args: Value| {
            Err("oops".into())
        });
        let result = t.call(json!({}));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "oops");
    }

    #[test]
    fn tool_clone_with_handler() {
        let t1 = Tool::new(
            "add",
            "Add two numbers",
            json!({"type": "object"}),
            |args: Value| {
                let a = args["a"].as_i64().unwrap_or(0);
                let b = args["b"].as_i64().unwrap_or(0);
                Ok(json!(a + b))
            },
        );
        let t2 = t1.clone_with_handler(|args: Value| {
            let a = args["a"].as_i64().unwrap_or(0);
            let b = args["b"].as_i64().unwrap_or(0);
            Ok(json!(a - b))
        });
        assert_eq!(t1.name, t2.name);
        assert_eq!(t1.description, t2.description);
        let result = t2.call(json!({"a": 5, "b": 3})).unwrap();
        assert_eq!(result, json!(2));
    }

    #[test]
    fn tool_debug_does_not_panic() {
        let t = Tool::new("x", "y", json!({}), |_args: Value| Ok(json!(null)));
        let dbg = format!("{:?}", t);
        assert!(dbg.contains("Tool"));
        assert!(dbg.contains("x"));
    }

    #[test]
    fn tool_schema_is_preserved() {
        let schema = json!({"type": "object", "properties": {"name": {"type": "string"}}});
        let t = Tool::new(" greet", "Say hi", schema.clone(), |_args: Value| {
            Ok(json!("hi"))
        });
        assert_eq!(t.input_schema, schema);
    }

    #[test]
    fn tool_handler_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Tool>();
    }
}
