// SPDX-License-Identifier: MIT OR Apache-2.0
//! agileplus-mcp-intent — MCP tool + HTTP service for converting user prompts
//! into AgilePlus intent graphs using the ontology schema.
//!
//! # Interfaces
//!
//! - **MCP Tool** (stdio JSON-RPC): `convert_prompt_to_intent_graph(prompt, options)`
//! - **HTTP API**: `POST /convert` → `{ "prompt": "...", "options": { ... } }`
//!
//! # Usage
//!
//! ```bash
//! # Run as MCP server
//! agileplus-mcp-intent mcp
//!
//! # Run HTTP API server
//! agileplus-mcp-intent http --port 8080
//!
//! # One-shot CLI conversion
//! agileplus-mcp-intent convert "Add dark mode to settings"
//!
//! # Convert and store in DB
//! agileplus-mcp-intent convert --store "Add dark mode to settings"
//! ```

pub mod converter;
pub mod http;
pub mod mcp;
pub mod storage;
pub mod types;
pub mod validator;

pub use converter::convert;
pub use types::{ConvertOptions, ConvertRequest, ConvertResponse, IntentGraph};
pub use validator::{full_validate, validate_graph};
