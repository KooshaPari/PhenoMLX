// SPDX-License-Identifier: MIT OR Apache-2.0
use agileplus_graph::{NodeType, RelType};

use crate::{Graph, PipelineError};

/// Export a `Graph` to a DOT string.
///
/// Maps:
///   - `NodeType::Feature` and `NodeType::WorkPackage` -> DOT nodes
///   - `RelType::DependsOn` and `RelType::Blocks` -> DOT edges
///
/// Node attributes are emitted from the `properties` JSON map.
/// Edge attributes are emitted from the relationship `properties` JSON map.
///
/// Non-relevant node types (Agent, Label, Project) and relationship types
/// (Owns, AssignedTo, Tagged, InProject) are skipped.
pub fn export(graph: &Graph) -> Result<String, PipelineError> {
    let mut lines = Vec::new();
    lines.push("digraph {".to_string());

    for node in graph.nodes.values() {
        match node.node_type {
            NodeType::Feature | NodeType::WorkPackage => {}
            _ => continue,
        }

        let label = node
            .properties
            .get("slug")
            .or_else(|| node.properties.get("title"))
            .or_else(|| node.properties.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| node.id.to_string());

        let mut attrs = properties_to_dot_attrs(&node.properties);
        // Always emit the UUID so parse_dot can restore node identity on round-trip.
        attrs.push(format!(r#"id="{}""#, node.id));
        let attr_str = format!(" [{}]", attrs.join(", "));

        lines.push(format!(r#"    "{label}"{attr_str};"#));
    }

    for rel in &graph.relationships {
        match rel.rel_type {
            RelType::DependsOn | RelType::Blocks => {}
            _ => continue,
        }

        let from_node = graph
            .get_node(rel.from_node_id)
            .ok_or_else(|| PipelineError::Export(format!("Missing node {}", rel.from_node_id)))?;
        let to_node = graph
            .get_node(rel.to_node_id)
            .ok_or_else(|| PipelineError::Export(format!("Missing node {}", rel.to_node_id)))?;

        let from_label = node_label(from_node);
        let to_label = node_label(to_node);

        let attrs = properties_to_dot_attrs(&rel.properties);
        let attr_str = if attrs.is_empty() {
            String::new()
        } else {
            format!(" [{}]", attrs.join(", "))
        };

        let edge_op = "->";
        lines.push(format!(
            r#"    "{from_label}" {edge_op} "{to_label}"{attr_str};"#
        ));
    }

    lines.push("}".to_string());
    Ok(lines.join("\n"))
}

fn node_label(node: &agileplus_graph::Node) -> String {
    node.properties
        .get("slug")
        .or_else(|| node.properties.get("title"))
        .or_else(|| node.properties.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| node.id.to_string())
}

fn properties_to_dot_attrs(properties: &serde_json::Value) -> Vec<String> {
    let mut attrs = Vec::new();
    if let Some(map) = properties.as_object() {
        for (key, value) in map {
            // Skip internal keys that are redundant in DOT
            if key == "slug" || key == "title" || key == "name" || key == "type" {
                continue;
            }
            let val = match value {
                serde_json::Value::String(s) => {
                    format!("\"{}\"", s.replace('"', "\\\"").replace('\n', "\\n"))
                }
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                _ => format!(
                    "\"{}\"",
                    value.to_string().replace('"', "\\\"").replace('\n', "\\n")
                ),
            };
            attrs.push(format!("{key}={val}"));
        }
    }
    attrs
}

#[cfg(test)]
mod tests {
    use super::*;
    use agileplus_graph::{Node, NodeType, RelType, Relationship};
    use serde_json::json;

    #[test]
    fn export_feature_and_work_package() {
        let mut graph = Graph::new();
        let n1 = Node::new(
            NodeType::Feature,
            json!({"slug": "feat-auth", "command": "echo auth", "retries": 2}),
        );
        let n2 = Node::new(
            NodeType::WorkPackage,
            json!({"slug": "wp-login", "command": "echo login", "timeout": 30}),
        );
        graph.add_node(n1.clone());
        graph.add_node(n2.clone());
        graph.add_relationship(Relationship::new(n1.id, n2.id, RelType::DependsOn));

        let dot = export(&graph).unwrap();
        assert!(dot.contains("digraph"));
        assert!(dot.contains("\"feat-auth\""));
        assert!(dot.contains("\"wp-login\""));
        assert!(dot.contains("command=\"echo auth\""));
        assert!(dot.contains("retries=2"));
        assert!(dot.contains("timeout=30"));
        assert!(dot.contains("->"));
    }

    #[test]
    fn export_skips_non_relevant_types() {
        let mut graph = Graph::new();
        let agent = Node::new(NodeType::Agent, json!({"slug": "agent-1"}));
        let label = Node::new(NodeType::Label, json!({"slug": "label-1"}));
        graph.add_node(agent.clone());
        graph.add_node(label.clone());

        let dot = export(&graph).unwrap();
        assert!(!dot.contains("agent-1"));
        assert!(!dot.contains("label-1"));
    }

    #[test]
    fn export_edge_with_guard() {
        let mut graph = Graph::new();
        let n1 = Node::new(
            NodeType::WorkPackage,
            json!({"slug": "a", "command": "echo a"}),
        );
        let n2 = Node::new(
            NodeType::WorkPackage,
            json!({"slug": "b", "command": "echo b"}),
        );
        let mut rel = Relationship::new(n1.id, n2.id, RelType::DependsOn);
        rel.properties = json!({"guard": "test -f ok", "weight": 1});
        graph.add_node(n1.clone());
        graph.add_node(n2.clone());
        graph.add_relationship(rel);

        let dot = export(&graph).unwrap();
        assert!(dot.contains("guard=\"test -f ok\""));
        assert!(dot.contains("weight=1"));
    }
}
