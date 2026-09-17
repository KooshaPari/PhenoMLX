// SPDX-License-Identifier: MIT OR Apache-2.0
//! Core types matching the AgilePlus intent graph ontology.
//!
//! Schema: `~/forge/prompt-corpus/agileplus-intent-ontology.json`

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Top-level intent graph document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub metadata: GraphMetadata,
}

/// A typed node in the intent graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub node_type: NodeType,
    pub dag_stage: DagStage,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub properties: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_id: Option<String>,
}

/// A directed edge between two nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relationship_type: RelationshipType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_map: Option<CanonicalMap>,
    pub meta: Meta,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub properties: Option<serde_json::Value>,
}

/// Metadata block for a graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphMetadata {
    pub version: String,
    pub schema_uri: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dag_valid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_system: Option<String>,
}

/// Meta block attached to every node and edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub timestamp: DateTime<Utc>,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

/// Canonical map for edge classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalMap {
    pub link_type: CanonicalLinkType,
    pub direction: Direction,
}

/// Ontology node types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    PR,
    Bug,
    Artifact,
}

/// DAG stage (lower-case variant of node type).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Relationship / edge types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Canonical link types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Edge direction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Forward,
    Reverse,
}

/// Node status values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Draft,
    Active,
    Completed,
    Deprecated,
    Rejected,
    Open,
    InProgress,
    Blocked,
    Deferred,
    Cancelled,
}

/// Conversion options for prompt -> intent graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ConvertOptions {
    #[serde(default = "default_auto_decompose")]
    pub auto_decompose: bool,
    #[serde(default = "default_max_features")]
    pub max_features: usize,
    #[serde(default = "default_store")]
    pub store: bool,
}

fn default_auto_decompose() -> bool {
    true
}

fn default_max_features() -> usize {
    5
}

fn default_store() -> bool {
    false
}

/// Input request for conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConvertRequest {
    pub prompt: String,
    #[serde(default)]
    pub options: ConvertOptions,
}

/// Successful conversion response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConvertResponse {
    pub graph: IntentGraph,
    pub summary: ConversionSummary,
}

/// Summary of what was produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversionSummary {
    pub node_count: usize,
    pub edge_count: usize,
    pub intent_title: String,
    pub features_generated: usize,
    pub plan_generated: bool,
    pub confidence: f64,
}

/// Error response payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.error)
    }
}

impl std::error::Error for ErrorResponse {}

impl IntentGraph {
    pub fn new(nodes: Vec<Node>, edges: Vec<Edge>) -> Self {
        let now = Utc::now();
        let node_count = nodes.len();
        let edge_count = edges.len();
        Self {
            nodes,
            edges,
            metadata: GraphMetadata {
                version: "1.0.0".to_string(),
                schema_uri: "https://phenotype.dev/schemas/agileplus-intent-ontology/v1.json"
                    .to_string(),
                created_at: now,
                updated_at: Some(now),
                node_count: Some(node_count),
                edge_count: Some(edge_count),
                dag_valid: Some(true),
                source_system: Some("agileplus-mcp-intent".to_string()),
            },
        }
    }
}

impl Node {
    pub fn builder(id: &str, node_type: NodeType, title: &str) -> NodeBuilder {
        NodeBuilder {
            id: id.to_string(),
            dag_stage: dag_stage_from_node_type(&node_type),
            node_type,
            title: title.to_string(),
            description: None,
            status: Status::Draft,
            tags: None,
            meta: Meta {
                timestamp: Utc::now(),
                source: "agent-inference".to_string(),
                confidence: Some(0.85),
                agent_id: Some("agileplus-mcp-intent".to_string()),
            },
            properties: None,
            table_ref: None,
            table_id: None,
        }
    }
}

pub struct NodeBuilder {
    id: String,
    node_type: NodeType,
    dag_stage: DagStage,
    title: String,
    description: Option<String>,
    status: Status,
    tags: Option<Vec<String>>,
    meta: Meta,
    properties: Option<serde_json::Value>,
    table_ref: Option<String>,
    table_id: Option<String>,
}

impl NodeBuilder {
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
    pub fn status(mut self, s: Status) -> Self {
        self.status = s;
        self
    }
    pub fn tags(mut self, t: Vec<String>) -> Self {
        self.tags = Some(t);
        self
    }
    pub fn meta(mut self, m: Meta) -> Self {
        self.meta = m;
        self
    }
    pub fn properties(mut self, p: serde_json::Value) -> Self {
        self.properties = Some(p);
        self
    }
    pub fn build(self) -> Node {
        Node {
            id: self.id,
            node_type: self.node_type,
            dag_stage: self.dag_stage,
            title: self.title,
            description: self.description,
            status: self.status,
            tags: self.tags,
            meta: self.meta,
            properties: self.properties,
            table_ref: self.table_ref,
            table_id: self.table_id,
        }
    }
}

fn dag_stage_from_node_type(nt: &NodeType) -> DagStage {
    match nt {
        NodeType::Intent => DagStage::Intent,
        NodeType::Plan => DagStage::Plan,
        NodeType::Feature => DagStage::Feature,
        NodeType::Story => DagStage::Story,
        NodeType::Task => DagStage::Task,
        NodeType::Spec => DagStage::Spec,
        NodeType::Commit => DagStage::Commit,
        NodeType::Test => DagStage::Test,
        NodeType::PR => DagStage::Pr,
        NodeType::Bug => DagStage::Bug,
        NodeType::Artifact => DagStage::Artifact,
    }
}

/// Round confidence to 2 decimal places for clean JSON output.
pub fn round_confidence(c: f64) -> f64 {
    (c * 100.0).round() / 100.0
}

/// Build an edge with full meta and canonical map.
pub fn make_edge(
    id: &str,
    source: &str,
    target: &str,
    rel: RelationshipType,
    link_type: CanonicalLinkType,
    direction: Direction,
    confidence: f64,
) -> Edge {
    Edge {
        id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        relationship_type: rel,
        canonical_map: Some(CanonicalMap {
            link_type,
            direction,
        }),
        meta: Meta {
            timestamp: Utc::now(),
            source: "agent-inference".to_string(),
            confidence: Some(round_confidence(confidence)),
            agent_id: Some("agileplus-mcp-intent".to_string()),
        },
        properties: None,
    }
}
