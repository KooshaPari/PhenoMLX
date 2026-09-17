// SPDX-License-Identifier: MIT OR Apache-2.0
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

// ==================== Data Types ====================

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct GraphMetadata {
    pub version: String,
    pub schema_uri: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub node_count: Option<usize>,
    #[serde(default)]
    pub edge_count: Option<usize>,
    #[serde(default)]
    pub dag_valid: Option<bool>,
    #[serde(default)]
    pub source_system: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct IntentGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub metadata: Option<GraphMetadata>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub dag_stage: DagStage,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub status: String,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    pub meta: Meta,
    #[serde(default)]
    pub properties: Option<serde_json::Value>,
    #[serde(default)]
    pub table_ref: Option<String>,
    #[serde(default)]
    pub table_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relationship_type: RelationshipType,
    #[serde(default)]
    pub canonical_map: Option<CanonicalMap>,
    pub meta: Meta,
    #[serde(default)]
    pub properties: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Meta {
    pub timestamp: String,
    pub source: String,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CanonicalMap {
    pub link_type: CanonicalLinkType,
    #[serde(default)]
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum NodeType {
    Intent,
    Plan,
    Feature,
    Story,
    Task,
    Spec,
    Commit,
    Test,
    #[serde(rename = "PR")]
    Pr,
    Bug,
    Artifact,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DagStage {
    Intent,
    Plan,
    Feature,
    Story,
    Task,
    Spec,
    Commit,
    Test,
    Pr,
    Bug,
    Artifact,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RelationshipType {
    Implements,
    Tests,
    Covers,
    #[serde(rename = "traces-to")]
    TracesTo,
    #[serde(rename = "derives-from")]
    DerivesFrom,
    Resolves,
    Blocks,
    #[serde(rename = "depends-on")]
    DependsOn,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalLinkType {
    ParentOf,
    ChildOf,
    DependsOn,
    Blocks,
    Implements,
    Verifies,
    References,
    Duplicates,
}

// ==================== Error Types ====================

#[derive(Debug, Error, PartialEq)]
pub enum ValidationError {
    #[error("cycle detected in intent graph: {0}")]
    CycleDetected(String),
    #[error("invalid edge constraint: edge {edge_id} has relationship '{relationship}' from '{source_type}' to '{target_type}', which is not allowed")]
    InvalidEdgeConstraint {
        edge_id: String,
        relationship: String,
        source_type: String,
        target_type: String,
    },
    #[error("incomplete node: {node_id} is an Intent but has no downstream Feature or Story")]
    IncompleteNode { node_id: String },
    #[error("invalid metadata on {context}: missing field '{field}'")]
    MissingMetadata { context: String, field: String },
    #[error("invalid metadata on {context}: confidence {confidence} is not in [0,1]")]
    InvalidConfidence { context: String, confidence: f64 },
    #[error("invalid DAG flow: edge {edge_id} goes from '{source_stage}' to '{target_stage}' with relationship '{relationship}', violating canonical flow")]
    InvalidDagFlow {
        edge_id: String,
        source_stage: String,
        target_stage: String,
        relationship: String,
    },
    #[error(
        "invalid canonical map: edge {edge_id} has unrecognized canonical link type '{link_type}'"
    )]
    InvalidCanonicalMap { edge_id: String, link_type: String },
    #[error("missing node: {node_id}")]
    MissingNode { node_id: String },
}

// ==================== Public API ====================

pub fn validate_intent_graph(graph: &IntentGraph) -> Result<(), ValidationError> {
    validate_dag_acyclicity(&graph.nodes, &graph.edges)?;
    validate_edge_constraints(&graph.nodes, &graph.edges)?;
    validate_node_completeness(&graph.nodes, &graph.edges)?;
    validate_metadata(&graph.nodes, &graph.edges)?;
    validate_dag_flow(&graph.nodes, &graph.edges)?;
    validate_canonical_map(&graph.edges)?;
    if let Some(ref meta) = graph.metadata {
        validate_graph_metadata(meta, graph.nodes.len(), graph.edges.len())?;
    }
    Ok(())
}

pub fn validate_graph_metadata(
    metadata: &GraphMetadata,
    node_count: usize,
    edge_count: usize,
) -> Result<(), ValidationError> {
    if metadata.version.trim().is_empty() {
        return Err(ValidationError::MissingMetadata {
            context: "graph.metadata".to_string(),
            field: "version".to_string(),
        });
    }
    if metadata.schema_uri.trim().is_empty() {
        return Err(ValidationError::MissingMetadata {
            context: "graph.metadata".to_string(),
            field: "schema_uri".to_string(),
        });
    }
    if metadata.created_at.trim().is_empty() {
        return Err(ValidationError::MissingMetadata {
            context: "graph.metadata".to_string(),
            field: "created_at".to_string(),
        });
    }
    if let Some(expected) = metadata.node_count {
        if expected != node_count {
            return Err(ValidationError::MissingMetadata {
                context: "graph.metadata".to_string(),
                field: format!("node_count mismatch: expected {expected}, got {node_count}"),
            });
        }
    }
    if let Some(expected) = metadata.edge_count {
        if expected != edge_count {
            return Err(ValidationError::MissingMetadata {
                context: "graph.metadata".to_string(),
                field: format!("edge_count mismatch: expected {expected}, got {edge_count}"),
            });
        }
    }
    Ok(())
}

// ==================== Validation Functions ====================

pub fn validate_dag_acyclicity(nodes: &[Node], edges: &[Edge]) -> Result<(), ValidationError> {
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    for edge in edges {
        adjacency
            .entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
    }

    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();

    for node in nodes {
        if !visited.contains(&node.id) {
            if let Some(cycle) = dfs_detect_cycle(
                &node.id,
                &adjacency,
                &mut visited,
                &mut rec_stack,
                &mut Vec::new(),
            ) {
                return Err(ValidationError::CycleDetected(cycle.join(" -> ")));
            }
        }
    }
    Ok(())
}

fn dfs_detect_cycle(
    node_id: &str,
    adjacency: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    rec_stack: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> Option<Vec<String>> {
    visited.insert(node_id.to_string());
    rec_stack.insert(node_id.to_string());
    path.push(node_id.to_string());

    if let Some(neighbors) = adjacency.get(node_id) {
        for neighbor in neighbors {
            if !visited.contains(neighbor) {
                if let Some(cycle) = dfs_detect_cycle(neighbor, adjacency, visited, rec_stack, path)
                {
                    return Some(cycle);
                }
            } else if rec_stack.contains(neighbor) {
                let cycle_start = path.iter().position(|id| id == neighbor).unwrap();
                let cycle = path[cycle_start..].to_vec();
                return Some(cycle);
            }
        }
    }

    rec_stack.remove(node_id);
    path.pop();
    None
}

pub fn validate_edge_constraints(nodes: &[Node], edges: &[Edge]) -> Result<(), ValidationError> {
    let node_types: HashMap<String, NodeType> =
        nodes.iter().map(|n| (n.id.clone(), n.node_type)).collect();

    for edge in edges {
        let source_type =
            node_types
                .get(&edge.source)
                .ok_or_else(|| ValidationError::MissingNode {
                    node_id: edge.source.clone(),
                })?;
        let target_type =
            node_types
                .get(&edge.target)
                .ok_or_else(|| ValidationError::MissingNode {
                    node_id: edge.target.clone(),
                })?;

        if edge.relationship_type == RelationshipType::TracesTo {
            continue; // wildcard allowed
        }

        if !is_edge_allowed(edge.relationship_type, *source_type, *target_type) {
            return Err(ValidationError::InvalidEdgeConstraint {
                edge_id: edge.id.clone(),
                relationship: format!("{:?}", edge.relationship_type),
                source_type: format!("{source_type:?}"),
                target_type: format!("{target_type:?}"),
            });
        }
    }
    Ok(())
}

fn is_edge_allowed(relationship: RelationshipType, from: NodeType, to: NodeType) -> bool {
    match relationship {
        RelationshipType::Implements => matches!(
            (from, to),
            (NodeType::Intent, NodeType::Feature)
                | (NodeType::Intent, NodeType::Story)
                | (NodeType::Intent, NodeType::Plan)
                | (NodeType::Plan, NodeType::Feature)
                | (NodeType::Feature, NodeType::Task)
                | (NodeType::Story, NodeType::Task)
                | (NodeType::Task, NodeType::Commit)
                | (NodeType::Spec, NodeType::Feature)
                | (NodeType::Spec, NodeType::Task)
        ),
        RelationshipType::Tests => matches!(
            (from, to),
            (NodeType::Feature, NodeType::Test)
                | (NodeType::Task, NodeType::Test)
                | (NodeType::Commit, NodeType::Test)
                | (NodeType::Pr, NodeType::Test)
                | (NodeType::Bug, NodeType::Test)
        ),
        RelationshipType::Covers => matches!(
            (from, to),
            (NodeType::Feature, NodeType::Test)
                | (NodeType::Task, NodeType::Test)
                | (NodeType::Feature, NodeType::Artifact)
                | (NodeType::Task, NodeType::Artifact)
                | (NodeType::Spec, NodeType::Feature)
        ),
        RelationshipType::TracesTo => true,
        RelationshipType::DerivesFrom => matches!(
            (from, to),
            (NodeType::Feature, NodeType::Intent)
                | (NodeType::Story, NodeType::Intent)
                | (NodeType::Story, NodeType::Feature)
                | (NodeType::Task, NodeType::Feature)
                | (NodeType::Task, NodeType::Story)
                | (NodeType::Task, NodeType::Bug)
        ),
        RelationshipType::Resolves => matches!(
            (from, to),
            (NodeType::Bug, NodeType::Commit)
                | (NodeType::Bug, NodeType::Pr)
                | (NodeType::Bug, NodeType::Task)
        ),
        RelationshipType::Blocks => matches!(
            (from, to),
            (NodeType::Task, NodeType::Task)
                | (NodeType::Bug, NodeType::Task)
                | (NodeType::Bug, NodeType::Pr)
                | (NodeType::Task, NodeType::Pr)
                | (NodeType::Pr, NodeType::Feature)
        ),
        RelationshipType::DependsOn => matches!(
            (from, to),
            (NodeType::Task, NodeType::Task)
                | (NodeType::Feature, NodeType::Feature)
                | (NodeType::Story, NodeType::Story)
                | (NodeType::Pr, NodeType::Pr)
                | (NodeType::Task, NodeType::Artifact)
        ),
    }
}

pub fn validate_node_completeness(nodes: &[Node], edges: &[Edge]) -> Result<(), ValidationError> {
    let intent_ids: HashSet<String> = nodes
        .iter()
        .filter(|n| n.node_type == NodeType::Intent)
        .map(|n| n.id.clone())
        .collect();

    let mut intent_with_downstream = HashSet::new();

    for edge in edges {
        if intent_ids.contains(&edge.source) {
            let target_type = nodes
                .iter()
                .find(|n| n.id == edge.target)
                .map(|n| n.node_type);
            if let Some(NodeType::Feature) | Some(NodeType::Story) = target_type {
                intent_with_downstream.insert(edge.source.clone());
            }
        }
    }

    for intent_id in intent_ids {
        if !intent_with_downstream.contains(&intent_id) {
            return Err(ValidationError::IncompleteNode { node_id: intent_id });
        }
    }
    Ok(())
}

pub fn validate_metadata(nodes: &[Node], edges: &[Edge]) -> Result<(), ValidationError> {
    for node in nodes {
        validate_meta(&node.meta, &format!("node {}", node.id))?;
    }
    for edge in edges {
        validate_meta(&edge.meta, &format!("edge {}", edge.id))?;
    }
    Ok(())
}

fn validate_meta(meta: &Meta, context: &str) -> Result<(), ValidationError> {
    if meta.timestamp.is_empty() {
        return Err(ValidationError::MissingMetadata {
            context: context.to_string(),
            field: "timestamp".to_string(),
        });
    }
    if meta.source.is_empty() {
        return Err(ValidationError::MissingMetadata {
            context: context.to_string(),
            field: "source".to_string(),
        });
    }
    if meta.agent_id.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        return Err(ValidationError::MissingMetadata {
            context: context.to_string(),
            field: "agent_id".to_string(),
        });
    }
    if let Some(confidence) = meta.confidence {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(ValidationError::InvalidConfidence {
                context: context.to_string(),
                confidence,
            });
        }
    }
    Ok(())
}

pub fn validate_dag_flow(nodes: &[Node], edges: &[Edge]) -> Result<(), ValidationError> {
    let stage_map: HashMap<String, DagStage> =
        nodes.iter().map(|n| (n.id.clone(), n.dag_stage)).collect();

    for edge in edges {
        let source_stage =
            stage_map
                .get(&edge.source)
                .ok_or_else(|| ValidationError::MissingNode {
                    node_id: edge.source.clone(),
                })?;
        let target_stage =
            stage_map
                .get(&edge.target)
                .ok_or_else(|| ValidationError::MissingNode {
                    node_id: edge.target.clone(),
                })?;

        let source_order = canonical_stage_order(*source_stage);
        let target_order = canonical_stage_order(*target_stage);

        let (source_order, target_order) = match (source_order, target_order) {
            (Some(s), Some(t)) => (s, t),
            _ => continue, // Bug or other non-canonical stage: skip
        };

        match edge.relationship_type {
            RelationshipType::Implements
            | RelationshipType::Tests
            | RelationshipType::Covers
            | RelationshipType::Resolves
            | RelationshipType::DependsOn => {
                if target_order < source_order {
                    return Err(ValidationError::InvalidDagFlow {
                        edge_id: edge.id.clone(),
                        source_stage: format!("{source_stage:?}"),
                        target_stage: format!("{target_stage:?}"),
                        relationship: format!("{:?}", edge.relationship_type),
                    });
                }
            }
            RelationshipType::DerivesFrom | RelationshipType::Blocks => {
                if target_order > source_order {
                    return Err(ValidationError::InvalidDagFlow {
                        edge_id: edge.id.clone(),
                        source_stage: format!("{source_stage:?}"),
                        target_stage: format!("{target_stage:?}"),
                        relationship: format!("{:?}", edge.relationship_type),
                    });
                }
            }
            RelationshipType::TracesTo => {
                // No restriction on traces-to
            }
        }
    }
    Ok(())
}

fn canonical_stage_order(stage: DagStage) -> Option<u8> {
    match stage {
        DagStage::Intent => Some(0),
        DagStage::Plan => Some(1),
        DagStage::Feature => Some(2),
        DagStage::Story => Some(2),
        DagStage::Task => Some(3),
        DagStage::Spec => Some(4),
        DagStage::Commit => Some(5),
        DagStage::Test => Some(6),
        DagStage::Pr => Some(7),
        DagStage::Artifact => Some(8),
        DagStage::Bug => None,
    }
}

pub fn validate_canonical_map(edges: &[Edge]) -> Result<(), ValidationError> {
    for edge in edges {
        if let Some(ref canonical_map) = edge.canonical_map {
            // Validate direction is valid if present
            if let Some(ref direction) = canonical_map.direction {
                if direction != "forward" && direction != "reverse" {
                    return Err(ValidationError::InvalidCanonicalMap {
                        edge_id: edge.id.clone(),
                        link_type: format!("invalid direction: {direction}"),
                    });
                }
            }

            // Validate consistency between relationship_type and canonical_map.link_type
            if let Err(msg) = validate_canonical_link_consistency(edge, canonical_map) {
                return Err(ValidationError::InvalidCanonicalMap {
                    edge_id: edge.id.clone(),
                    link_type: msg,
                });
            }
        }
    }
    Ok(())
}

fn validate_canonical_link_consistency(
    edge: &Edge,
    canonical_map: &CanonicalMap,
) -> Result<(), String> {
    let expected = match edge.relationship_type {
        RelationshipType::Implements => {
            vec![
                CanonicalLinkType::Implements,
                CanonicalLinkType::ParentOf,
                CanonicalLinkType::ChildOf,
            ]
        }
        RelationshipType::Tests => {
            vec![CanonicalLinkType::Verifies, CanonicalLinkType::References]
        }
        RelationshipType::Covers => {
            vec![
                CanonicalLinkType::Verifies,
                CanonicalLinkType::References,
                CanonicalLinkType::ParentOf,
            ]
        }
        RelationshipType::TracesTo => {
            vec![
                CanonicalLinkType::References,
                CanonicalLinkType::Duplicates,
                CanonicalLinkType::DependsOn,
            ]
        }
        RelationshipType::DerivesFrom => {
            vec![
                CanonicalLinkType::ParentOf,
                CanonicalLinkType::ChildOf,
                CanonicalLinkType::DependsOn,
            ]
        }
        RelationshipType::Resolves => {
            vec![
                CanonicalLinkType::Implements,
                CanonicalLinkType::Blocks,
                CanonicalLinkType::References,
            ]
        }
        RelationshipType::Blocks => {
            vec![CanonicalLinkType::Blocks, CanonicalLinkType::DependsOn]
        }
        RelationshipType::DependsOn => {
            vec![
                CanonicalLinkType::DependsOn,
                CanonicalLinkType::ParentOf,
                CanonicalLinkType::ChildOf,
            ]
        }
    };

    if !expected.contains(&canonical_map.link_type) {
        return Err(format!("{:?}", canonical_map.link_type));
    }
    Ok(())
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: &str, node_type: NodeType, dag_stage: DagStage) -> Node {
        Node {
            id: id.to_string(),
            node_type,
            dag_stage,
            title: id.to_string(),
            description: None,
            status: "draft".to_string(),
            tags: None,
            meta: make_meta(),
            properties: None,
            table_ref: None,
            table_id: None,
        }
    }

    fn make_edge(
        id: &str,
        source: &str,
        target: &str,
        relationship_type: RelationshipType,
    ) -> Edge {
        Edge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            relationship_type,
            canonical_map: None,
            meta: make_meta(),
            properties: None,
        }
    }

    fn make_meta() -> Meta {
        Meta {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            source: "test".to_string(),
            agent_id: Some("agent-1".to_string()),
            confidence: Some(0.95),
        }
    }

    #[test]
    fn acyclic_graph_passes() {
        let nodes = vec![
            make_node("n1", NodeType::Intent, DagStage::Intent),
            make_node("n2", NodeType::Plan, DagStage::Plan),
        ];
        let edges = vec![make_edge("e1", "n1", "n2", RelationshipType::Implements)];
        assert!(validate_dag_acyclicity(&nodes, &edges).is_ok());
    }

    #[test]
    fn cycle_detected() {
        let nodes = vec![
            make_node("n1", NodeType::Task, DagStage::Task),
            make_node("n2", NodeType::Task, DagStage::Task),
            make_node("n3", NodeType::Task, DagStage::Task),
        ];
        let edges = vec![
            make_edge("e1", "n1", "n2", RelationshipType::DependsOn),
            make_edge("e2", "n2", "n3", RelationshipType::DependsOn),
            make_edge("e3", "n3", "n1", RelationshipType::DependsOn),
        ];
        let result = validate_dag_acyclicity(&nodes, &edges);
        assert!(matches!(result, Err(ValidationError::CycleDetected(_))));
    }

    #[test]
    fn valid_edge_constraint() {
        let nodes = vec![
            make_node("n1", NodeType::Intent, DagStage::Intent),
            make_node("n2", NodeType::Feature, DagStage::Feature),
        ];
        let edges = vec![make_edge("e1", "n1", "n2", RelationshipType::Implements)];
        assert!(validate_edge_constraints(&nodes, &edges).is_ok());
    }

    #[test]
    fn invalid_edge_constraint() {
        let nodes = vec![
            make_node("n1", NodeType::Intent, DagStage::Intent),
            make_node("n2", NodeType::Commit, DagStage::Commit),
        ];
        let edges = vec![make_edge("e1", "n1", "n2", RelationshipType::Implements)];
        let result = validate_edge_constraints(&nodes, &edges);
        assert!(matches!(
            result,
            Err(ValidationError::InvalidEdgeConstraint { .. })
        ));
    }

    #[test]
    fn traces_to_wildcard_allowed() {
        let nodes = vec![
            make_node("n1", NodeType::Intent, DagStage::Intent),
            make_node("n2", NodeType::Artifact, DagStage::Artifact),
        ];
        let edges = vec![make_edge("e1", "n1", "n2", RelationshipType::TracesTo)];
        assert!(validate_edge_constraints(&nodes, &edges).is_ok());
    }

    #[test]
    fn intent_without_downstream_fails() {
        let nodes = vec![make_node("n1", NodeType::Intent, DagStage::Intent)];
        let edges = vec![];
        let result = validate_node_completeness(&nodes, &edges);
        assert!(matches!(
            result,
            Err(ValidationError::IncompleteNode { .. })
        ));
    }

    #[test]
    fn intent_with_feature_downstream_passes() {
        let nodes = vec![
            make_node("n1", NodeType::Intent, DagStage::Intent),
            make_node("n2", NodeType::Feature, DagStage::Feature),
        ];
        let edges = vec![make_edge("e1", "n1", "n2", RelationshipType::Implements)];
        assert!(validate_node_completeness(&nodes, &edges).is_ok());
    }

    #[test]
    fn intent_with_story_downstream_passes() {
        let nodes = vec![
            make_node("n1", NodeType::Intent, DagStage::Intent),
            make_node("n2", NodeType::Story, DagStage::Story),
        ];
        let edges = vec![make_edge("e1", "n1", "n2", RelationshipType::Implements)];
        assert!(validate_node_completeness(&nodes, &edges).is_ok());
    }

    #[test]
    fn valid_metadata_passes() {
        let nodes = vec![make_node("n1", NodeType::Intent, DagStage::Intent)];
        let edges = vec![];
        assert!(validate_metadata(&nodes, &edges).is_ok());
    }

    #[test]
    fn missing_agent_id_fails() {
        let nodes = vec![Node {
            meta: Meta {
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                source: "test".to_string(),
                agent_id: None,
                confidence: None,
            },
            ..make_node("n1", NodeType::Intent, DagStage::Intent)
        }];
        let edges = vec![];
        let result = validate_metadata(&nodes, &edges);
        assert!(matches!(
            result,
            Err(ValidationError::MissingMetadata { .. })
        ));
    }

    #[test]
    fn invalid_confidence_fails() {
        let nodes = vec![Node {
            meta: Meta {
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                source: "test".to_string(),
                agent_id: Some("agent-1".to_string()),
                confidence: Some(1.5),
            },
            ..make_node("n1", NodeType::Intent, DagStage::Intent)
        }];
        let edges = vec![];
        let result = validate_metadata(&nodes, &edges);
        assert!(matches!(
            result,
            Err(ValidationError::InvalidConfidence { .. })
        ));
    }

    #[test]
    fn canonical_flow_forward_passes() {
        let nodes = vec![
            make_node("n1", NodeType::Intent, DagStage::Intent),
            make_node("n2", NodeType::Plan, DagStage::Plan),
        ];
        let edges = vec![make_edge("e1", "n1", "n2", RelationshipType::Implements)];
        assert!(validate_dag_flow(&nodes, &edges).is_ok());
    }

    #[test]
    fn canonical_flow_backward_fails() {
        let nodes = vec![
            make_node("n1", NodeType::Task, DagStage::Task),
            make_node("n2", NodeType::Intent, DagStage::Intent),
        ];
        let edges = vec![make_edge("e1", "n1", "n2", RelationshipType::Implements)];
        let result = validate_dag_flow(&nodes, &edges);
        assert!(matches!(
            result,
            Err(ValidationError::InvalidDagFlow { .. })
        ));
    }

    #[test]
    fn derives_from_backward_passes() {
        let nodes = vec![
            make_node("n1", NodeType::Feature, DagStage::Feature),
            make_node("n2", NodeType::Intent, DagStage::Intent),
        ];
        let edges = vec![make_edge("e1", "n1", "n2", RelationshipType::DerivesFrom)];
        assert!(validate_dag_flow(&nodes, &edges).is_ok());
    }

    #[test]
    fn valid_canonical_map_passes() {
        let mut edge = make_edge("e1", "n1", "n2", RelationshipType::Implements);
        edge.canonical_map = Some(CanonicalMap {
            link_type: CanonicalLinkType::Implements,
            direction: Some("forward".to_string()),
        });
        assert!(validate_canonical_map(&[edge]).is_ok());
    }

    #[test]
    fn invalid_canonical_map_direction_fails() {
        let mut edge = make_edge("e1", "n1", "n2", RelationshipType::Implements);
        edge.canonical_map = Some(CanonicalMap {
            link_type: CanonicalLinkType::Implements,
            direction: Some("sideways".to_string()),
        });
        let result = validate_canonical_map(&[edge]);
        assert!(matches!(
            result,
            Err(ValidationError::InvalidCanonicalMap { .. })
        ));
    }

    #[test]
    fn invalid_canonical_map_link_type_fails() {
        let mut edge = make_edge("e1", "n1", "n2", RelationshipType::Implements);
        edge.canonical_map = Some(CanonicalMap {
            link_type: CanonicalLinkType::Verifies,
            direction: Some("forward".to_string()),
        });
        let result = validate_canonical_map(&[edge]);
        assert!(matches!(
            result,
            Err(ValidationError::InvalidCanonicalMap { .. })
        ));
    }

    #[test]
    fn validate_intent_graph_full_pass() {
        let nodes = vec![
            make_node("Intent#auth", NodeType::Intent, DagStage::Intent),
            make_node("Feature#oauth2", NodeType::Feature, DagStage::Feature),
            make_node("Task#impl", NodeType::Task, DagStage::Task),
            make_node("Commit#abc123", NodeType::Commit, DagStage::Commit),
            make_node("Test#unit", NodeType::Test, DagStage::Test),
        ];
        let edges = vec![
            make_edge(
                "e1",
                "Intent#auth",
                "Feature#oauth2",
                RelationshipType::Implements,
            ),
            make_edge(
                "e2",
                "Feature#oauth2",
                "Task#impl",
                RelationshipType::Implements,
            ),
            make_edge(
                "e3",
                "Task#impl",
                "Commit#abc123",
                RelationshipType::Implements,
            ),
            make_edge("e4", "Commit#abc123", "Test#unit", RelationshipType::Tests),
        ];
        let graph = IntentGraph {
            nodes,
            edges,
            metadata: None,
        };
        assert!(validate_intent_graph(&graph).is_ok());
    }

    #[test]
    fn validate_intent_graph_cycle_fails() {
        let nodes = vec![
            make_node("n1", NodeType::Task, DagStage::Task),
            make_node("n2", NodeType::Task, DagStage::Task),
        ];
        let edges = vec![
            make_edge("e1", "n1", "n2", RelationshipType::DependsOn),
            make_edge("e2", "n2", "n1", RelationshipType::DependsOn),
        ];
        let graph = IntentGraph {
            nodes,
            edges,
            metadata: None,
        };
        let result = validate_intent_graph(&graph);
        assert!(matches!(result, Err(ValidationError::CycleDetected(_))));
    }
}
