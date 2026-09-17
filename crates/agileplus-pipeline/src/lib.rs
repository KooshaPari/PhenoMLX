// SPDX-License-Identifier: MIT OR Apache-2.0
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use agileplus_graph::{Node, Relationship};

/// In-memory graph container for pipeline execution.
/// Holds nodes and relationships without the async GraphStore trait overhead.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: HashMap<Uuid, Node>,
    pub relationships: Vec<Relationship>,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.id, node);
    }

    pub fn add_relationship(&mut self, rel: Relationship) {
        self.relationships.push(rel);
    }

    pub fn get_node(&self, id: Uuid) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn neighbors(&self, node_id: Uuid) -> Vec<&Relationship> {
        self.relationships
            .iter()
            .filter(|r| r.from_node_id == node_id || r.to_node_id == node_id)
            .collect()
    }

    pub fn outgoing(&self, node_id: Uuid) -> Vec<&Relationship> {
        self.relationships
            .iter()
            .filter(|r| r.from_node_id == node_id)
            .collect()
    }

    pub fn incoming(&self, node_id: Uuid) -> Vec<&Relationship> {
        self.relationships
            .iter()
            .filter(|r| r.to_node_id == node_id)
            .collect()
    }

    /// Return the IDs of nodes that `node_id` depends on (outgoing DEPENDS_ON edges).
    pub fn dependencies(&self, node_id: Uuid) -> Vec<Uuid> {
        self.outgoing(node_id)
            .into_iter()
            .filter(|r| r.rel_type == agileplus_graph::RelType::DependsOn)
            .map(|r| r.to_node_id)
            .collect()
    }

    /// Return the IDs of nodes that block `node_id` (incoming BLOCKS edges).
    pub fn blockers(&self, node_id: Uuid) -> Vec<Uuid> {
        self.incoming(node_id)
            .into_iter()
            .filter(|r| r.rel_type == agileplus_graph::RelType::Blocks)
            .map(|r| r.from_node_id)
            .collect()
    }
}

pub mod dot_export;
pub mod dot_parser;
pub mod executor;
pub mod resource;

use executor::Executor;
use resource::ResourceLimits;

/// Pipeline owns a graph and can execute it with configurable resource limits.
pub struct Pipeline {
    pub graph: Graph,
    pub executor: Executor,
    pub resource_limits: ResourceLimits,
}

impl Pipeline {
    pub fn new(graph: Graph) -> Self {
        Self {
            graph,
            executor: Executor::default(),
            resource_limits: ResourceLimits::default(),
        }
    }

    pub fn with_limits(graph: Graph, limits: ResourceLimits) -> Self {
        Self {
            graph,
            executor: Executor::default(),
            resource_limits: limits,
        }
    }

    pub async fn execute(&self) -> anyhow::Result<executor::ExecutionResult> {
        self.executor
            .execute(&self.graph, &self.resource_limits)
            .await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("DOT parse error: {0}")]
    DotParse(String),
    #[error("Execution error: {0}")]
    Execution(String),
    #[error("Export error: {0}")]
    Export(String),
    #[error("Graph error: {0}")]
    Graph(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use agileplus_graph::{NodeType, RelType};

    #[test]
    fn graph_round_trip_via_dot() {
        let mut graph = Graph::new();
        let n1 = Node::new(
            NodeType::Feature,
            serde_json::json!({"slug": "feat-1", "command": "echo hello"}),
        );
        let n2 = Node::new(
            NodeType::WorkPackage,
            serde_json::json!({"slug": "wp-1", "command": "echo world"}),
        );
        graph.add_node(n1.clone());
        graph.add_node(n2.clone());
        graph.add_relationship(Relationship::new(n1.id, n2.id, RelType::DependsOn));

        let dot = dot_export::export(&graph).unwrap();
        let parsed = dot_parser::parse_dot(&dot).unwrap();

        assert_eq!(parsed.nodes.len(), 2);
        assert_eq!(parsed.relationships.len(), 1);
        assert!(parsed.nodes.contains_key(&n1.id));
        assert!(parsed.nodes.contains_key(&n2.id));
        let rel = &parsed.relationships[0];
        assert_eq!(rel.from_node_id, n1.id);
        assert_eq!(rel.to_node_id, n2.id);
        assert_eq!(rel.rel_type, RelType::DependsOn);
    }

    #[test]
    fn graph_round_trip_preserves_embedded_quotes() {
        // Integration regression: a value containing an embedded `"` must
        // survive `dot_export -> parse_dot`. 764 already covers export-side
        // and parse-side separately; this test exercises the full path.
        let mut graph = Graph::new();
        let n1 = Node::new(
            NodeType::WorkPackage,
            serde_json::json!({
                "slug": "auth",
                "command": "echo \"auth required\"",
            }),
        );
        let n2 = Node::new(NodeType::WorkPackage, serde_json::json!({"slug": "deploy"}));
        graph.add_node(n1.clone());
        graph.add_node(n2.clone());
        graph.add_relationship(Relationship::new(n1.id, n2.id, RelType::DependsOn));

        let dot = dot_export::export(&graph).unwrap();
        let parsed = dot_parser::parse_dot(&dot)
            .expect("round-trip must not fail on embedded quotes");

        assert!(parsed.nodes.contains_key(&n1.id));
        assert!(parsed.nodes.contains_key(&n2.id));
        assert_eq!(
            parsed.nodes[&n1.id].properties["command"],
            "echo \"auth required\""
        );

        let rel = &parsed.relationships[0];
        assert_eq!(rel.from_node_id, n1.id);
        assert_eq!(rel.to_node_id, n2.id);
    }

    #[tokio::test]
    async fn topo_execution_on_three_node_graph() {
        let mut graph = Graph::new();
        let n1 = Node::new(
            NodeType::WorkPackage,
            serde_json::json!({
                "slug": "a",
                "command": "echo a",
                "retries": 0,
            }),
        );
        let n2 = Node::new(
            NodeType::WorkPackage,
            serde_json::json!({
                "slug": "b",
                "command": "echo b",
                "retries": 0,
            }),
        );
        let n3 = Node::new(
            NodeType::WorkPackage,
            serde_json::json!({
                "slug": "c",
                "command": "echo c",
                "retries": 0,
            }),
        );

        graph.add_node(n1.clone());
        graph.add_node(n2.clone());
        graph.add_node(n3.clone());

        // n1 depends on n2, n2 depends on n3
        // execution order: n3, n2, n1
        graph.add_relationship(Relationship::new(n1.id, n2.id, RelType::DependsOn));
        graph.add_relationship(Relationship::new(n2.id, n3.id, RelType::DependsOn));

        let pipeline = Pipeline::new(graph);
        let result = pipeline.execute().await.unwrap();

        assert_eq!(result.node_outputs.len(), 3);
        assert!(result.node_outputs[&n3.id].success, "n3 (no-dep leaf) should succeed");
        assert!(result.node_outputs[&n2.id].success, "n2 (depends on n3) should succeed");
        assert!(result.node_outputs[&n1.id].success, "n1 (depends on n2) should succeed");

        // Verify topological order: n3 finished before n2, n2 before n1
        assert!(result.node_outputs[&n3.id].finished_at <= result.node_outputs[&n2.id].started_at);
        assert!(result.node_outputs[&n2.id].finished_at <= result.node_outputs[&n1.id].started_at);
    }
}
