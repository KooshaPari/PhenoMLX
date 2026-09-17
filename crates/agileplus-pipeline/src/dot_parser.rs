// SPDX-License-Identifier: MIT OR Apache-2.0
use std::collections::HashMap;

use regex::Regex;
use uuid::Uuid;

use agileplus_graph::{Node, NodeType, RelType, Relationship};

use crate::{Graph, PipelineError};

/// Parse a simplified DOT digraph into our `Graph` types.
///
/// Supported syntax:
/// ```text
/// digraph {
///     "node-1" [command="echo hello", retries=3, timeout=60];
///     "node-1" -> "node-2" [guard="test -f ok", weight=1];
/// }
/// ```
///
/// Node attributes:
///   - `command="..."`       shell command to execute
///   - `working_dir="..."`   working directory
///   - `retries=N`           number of retries on failure
///   - `timeout=N`           timeout in seconds
///   - `cpus=N`              CPU limit
///   - `mem="..."`           memory limit
///
/// Edge attributes:
///   - `guard="..."`         shell predicate; edge is traversed only if exit code == 0
///   - `weight=N`            edge weight
pub fn parse_dot(dot: &str) -> Result<Graph, PipelineError> {
    let mut graph = Graph::new();
    let mut id_map: HashMap<String, Uuid> = HashMap::new();

    let node_re = Regex::new(
        r#"(?x)
        ^\s*"?([^"\s;]+)"?\s*\[\s*([^\]]*?)\s*\]\s*;?
        "#,
    )
    .map_err(|e| PipelineError::DotParse(e.to_string()))?;

    let edge_re = Regex::new(
        r#"(?x)
        ^\s*"?([^"\s;]+)"?\s*->\s*"?([^"\s;]+)"?\s*(?:\[\s*([^\]]*?)\s*\])?\s*;?
        "#,
    )
    .map_err(|e| PipelineError::DotParse(e.to_string()))?;

    // Pass 1: parse nodes
    for line in dot.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with("#") {
            continue;
        }
        // Skip digraph / graph / braces
        if line.starts_with("digraph") || line.starts_with("graph") {
            continue;
        }
        if line == "{" || line == "}" {
            continue;
        }
        // Skip edge lines — they are parsed in Pass 2.
        // CRITICAL: do NOT use `line.contains("->")` here. Node attributes may
        // legitimately contain `->` inside quoted values (e.g.
        // `command="echo a -> b"`); substring matching would silently drop
        // those node declarations. Use the edge regex's anchored match
        // instead, which only fires on actual DOT edge syntax at the start
        // of the line.
        if edge_re.is_match(line) {
            continue;
        }

        if let Some(caps) = node_re.captures(line) {
            let name = caps[1].trim().to_string();
            let attrs = &caps[2];
            let properties = parse_attributes(attrs);
            let node_type = infer_node_type(&properties);
            let id = *id_map.entry(name.clone()).or_insert_with(|| {
                // If properties already contain a UUID, reuse it; otherwise generate new.
                properties
                    .get("id")
                    .and_then(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                    .unwrap_or_else(Uuid::new_v4)
            });
            let node = Node::with_id(id, node_type, properties);
            graph.add_node(node);
        }
    }

    // Pass 2: parse edges
    for line in dot.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with("#") {
            continue;
        }
        if line.starts_with("digraph") || line.starts_with("graph") || line == "{" || line == "}" {
            continue;
        }

        if let Some(caps) = edge_re.captures(line) {
            let from_name = caps[1].trim().to_string();
            let to_name = caps[2].trim().to_string();
            let attrs = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            let properties = parse_attributes(attrs);

            let from_id = *id_map
                .get(&from_name)
                .ok_or_else(|| PipelineError::DotParse(format!("Unknown node: {from_name}")))?;
            let to_id = *id_map
                .get(&to_name)
                .ok_or_else(|| PipelineError::DotParse(format!("Unknown node: {to_name}")))?;

            let rel_type = infer_rel_type(&properties);
            let mut rel = Relationship::new(from_id, to_id, rel_type);
            rel.properties = properties;
            graph.add_relationship(rel);
        }
    }

    Ok(graph)
}

fn parse_attributes(attr_str: &str) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let re = Regex::new(
        r#"(?x)
        (\w+)\s*=\s*
        (?:
            "((?:[^"\\]|\\.)*)" |
            ([0-9]+)
        )
    "#,
    )
    .unwrap();

    for caps in re.captures_iter(attr_str) {
        let key = caps[1].to_string();
        let value = if let Some(val) = caps.get(2) {
            // Quoted string — unescape DOT backslash-escapes
            let raw = val.as_str();
            let mut out = String::with_capacity(raw.len());
            let mut chars = raw.chars();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    if let Some(next) = chars.next() {
                        out.push(next);
                    }
                    // stray trailing backslash: drop it
                } else {
                    out.push(c);
                }
            }
            serde_json::Value::String(out)
        } else if let Some(val) = caps.get(3) {
            // Integer
            val.as_str()
                .parse::<i64>()
                .map(serde_json::Value::Number)
                .unwrap_or_else(|_| serde_json::Value::String(val.as_str().to_string()))
        } else {
            continue;
        };
        map.insert(key, value);
    }

    serde_json::Value::Object(map)
}

fn infer_node_type(properties: &serde_json::Value) -> NodeType {
    match properties.get("type").and_then(|v| v.as_str()) {
        Some("Feature") => NodeType::Feature,
        Some("WorkPackage") => NodeType::WorkPackage,
        Some("Agent") => NodeType::Agent,
        Some("Label") => NodeType::Label,
        Some("Project") => NodeType::Project,
        _ => {
            // Default heuristic: if it has a `command` attribute, treat as WorkPackage
            if properties.get("command").is_some() {
                NodeType::WorkPackage
            } else {
                NodeType::Feature
            }
        }
    }
}

fn infer_rel_type(properties: &serde_json::Value) -> RelType {
    match properties.get("type").and_then(|v| v.as_str()) {
        Some("DependsOn") => RelType::DependsOn,
        Some("Blocks") => RelType::Blocks,
        Some("Owns") => RelType::Owns,
        Some("AssignedTo") => RelType::AssignedTo,
        Some("Tagged") => RelType::Tagged,
        Some("InProject") => RelType::InProject,
        _ => {
            // Default heuristic: if it has a `guard` attribute, treat as DependsOn
            RelType::DependsOn
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_simple_digraph() {
        let dot = r#"
            digraph {
                "build" [command="cargo build", retries=2, timeout=120, cpus=2, mem="1G"];
                "test" [command="cargo test", retries=1, timeout=300];
                "deploy" [command="./deploy.sh", retries=0];
                build -> test [guard="test -f target/debug/app", weight=1];
                test -> deploy [weight=1];
            }
        "#;

        let graph = parse_dot(dot).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.relationships.len(), 2);

        let build_node = graph
            .nodes
            .values()
            .find(|n| n.properties.get("command") == Some(&json!("cargo build")))
            .unwrap();
        assert_eq!(build_node.properties["retries"], 2);
        assert_eq!(build_node.properties["timeout"], 120);
        assert_eq!(build_node.properties["cpus"], 2);
        assert_eq!(build_node.properties["mem"], "1G");

        let rel = &graph.relationships[0];
        assert_eq!(rel.properties["guard"], "test -f target/debug/app");
        assert_eq!(rel.properties["weight"], 1);
    }

    #[test]
    fn parse_nodes_without_quotes() {
        let dot = r#"
            digraph {
                a [command="echo a"];
                b [command="echo b"];
                a -> b;
            }
        "#;
        let graph = parse_dot(dot).unwrap();
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.relationships.len(), 1);
    }

    #[test]
    fn parse_working_dir_attribute() {
        let dot = r#"
            digraph {
                "setup" [command="./setup.sh", working_dir="/tmp", retries=3, timeout=60];
            }
        "#;
        let graph = parse_dot(dot).unwrap();
        let node = graph.nodes.values().next().unwrap();
        assert_eq!(node.properties["working_dir"], "/tmp");
        assert_eq!(node.properties["retries"], 3);
        assert_eq!(node.properties["timeout"], 60);
    }

    /// Regression test for codeant-ai finding #7 (Major):
    /// Node attribute values may legitimately contain `->` (e.g. a shell
    /// command like `echo a -> b`). The previous `line.contains("->")` edge
    /// skip was too broad and silently dropped such nodes in Pass 1.
    /// We must use the anchored `edge_re` so node declarations survive.
    #[test]
    fn parses_node_with_arrow_inside_attribute_value() {
        let dot = r#"
            digraph {
                "build" [command="echo a -> b", retries=1];
                "test" [command="cargo test"];
                build -> test;
            }
        "#;
        let graph = parse_dot(dot).expect("parse must not skip node with arrow in attribute");
        assert_eq!(
            graph.nodes.len(),
            2,
            "both nodes must be present even though 'build' has -> in its command"
        );
        let build = graph
            .nodes
            .values()
            .find(|n| n.properties.get("command") == Some(&json!("echo a -> b")))
            .expect("build node with arrow-in-command must be present");
        assert_eq!(build.properties["retries"], 1);
        assert_eq!(graph.relationships.len(), 1);
        assert_eq!(graph.relationships[0].from_node_id, build.id);
    }
}
