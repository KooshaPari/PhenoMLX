// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ontology schema validation.
//!
//! Validates generated intent graphs against the embedded JSON Schema
//! (`agileplus-intent-ontology.json`).

use jsonschema::Validator;
use serde_json::Value;

use crate::types::{ConvertResponse, ErrorResponse, IntentGraph};

/// Path to the ontology schema (resolved at runtime).
const _ONTOLOGY_SCHEMA_PATH: &str = "~/forge/prompt-corpus/agileplus-intent-ontology.json";

/// Embedded fallback schema (minimal subset to avoid runtime file dependency).
pub const EMBEDDED_SCHEMA: &str = include_str!("../ontology.json");

fn get_validator() -> Result<Validator, String> {
    let schema: Value =
        serde_json::from_str(EMBEDDED_SCHEMA).map_err(|e| format!("schema parse failed: {e}"))?;
    Validator::new(&schema).map_err(|e| format!("schema compile failed: {e}"))
}

/// Validate an intent graph against the ontology schema.
pub fn validate_graph(graph: &IntentGraph) -> Result<(), Vec<String>> {
    let validator = match get_validator() {
        Ok(v) => v,
        Err(e) => return Err(vec![e]),
    };

    let value = match serde_json::to_value(graph) {
        Ok(v) => v,
        Err(e) => return Err(vec![format!("serialization failed: {e}")]),
    };

    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|e| format!("{}: {}", e.instance_path, e))
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate and return the response, or an error payload.
pub fn validate_and_wrap(response: ConvertResponse) -> Result<ConvertResponse, ErrorResponse> {
    match validate_graph(&response.graph) {
        Ok(()) => Ok(response),
        Err(errors) => Err(ErrorResponse {
            error: errors.join("; "),
            code: "ONTOLOGY_VALIDATION_ERROR".to_string(),
            details: Some(serde_json::json!({ "validation_errors": errors })),
        }),
    }
}

/// Quick validation of a single node id format.
pub fn validate_node_id(id: &str) -> Result<(), String> {
    let re = regex::Regex::new(r"^[A-Z][a-z]+#[a-z0-9\-]+$").unwrap();
    if re.is_match(id) {
        Ok(())
    } else {
        Err(format!(
            "node id '{id}' does not match pattern [A-Z][a-z]+#[a-z0-9\\-]+"
        ))
    }
}

/// Quick validation of an edge meta block.
pub fn validate_edge_meta(edge: &crate::types::Edge) -> Vec<String> {
    let mut errors = vec![];
    if edge.meta.timestamp.to_rfc3339().is_empty() {
        errors.push(format!("edge {}: missing timestamp", edge.id));
    }
    if edge.meta.source.is_empty() {
        errors.push(format!("edge {}: missing source", edge.id));
    }
    if edge
        .meta
        .agent_id
        .as_ref()
        .map(|s| s.is_empty())
        .unwrap_or(true)
    {
        errors.push(format!("edge {}: missing agent_id", edge.id));
    }
    if let Some(c) = edge.meta.confidence {
        if !(0.0..=1.0).contains(&c) {
            errors.push(format!("edge {}: confidence {c} out of range", edge.id));
        }
    }
    errors
}

/// Full validation: schema + node id patterns + edge meta requirements.
pub fn full_validate(response: &ConvertResponse) -> Result<(), Vec<String>> {
    let mut errors = vec![];

    // 1. Schema validation
    match validate_graph(&response.graph) {
        Ok(()) => {}
        Err(e) => errors.extend(e),
    }

    // 2. Node id patterns
    for node in &response.graph.nodes {
        if let Err(e) = validate_node_id(&node.id) {
            errors.push(e);
        }
    }

    // 3. Edge meta requirements
    for edge in &response.graph.edges {
        errors.extend(validate_edge_meta(edge));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
