use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeType {
    Feature,
    WorkPackage,
    Agent,
    Label,
    Project,
}

impl NodeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeType::Feature => "Feature",
            NodeType::WorkPackage => "WorkPackage",
            NodeType::Agent => "Agent",
            NodeType::Label => "Label",
            NodeType::Project => "Project",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelType {
    Owns,
    AssignedTo,
    DependsOn,
    Blocks,
    Tagged,
    InProject,
}

impl RelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RelType::Owns => "OWNS",
            RelType::AssignedTo => "ASSIGNED_TO",
            RelType::DependsOn => "DEPENDS_ON",
            RelType::Blocks => "BLOCKS",
            RelType::Tagged => "TAGGED",
            RelType::InProject => "IN_PROJECT",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: uuid::Uuid,
    pub node_type: NodeType,
    pub properties: serde_json::Value,
}

impl Node {
    pub fn new(node_type: NodeType, properties: serde_json::Value) -> Self {
        Self::with_id(uuid::Uuid::new_v4(), node_type, properties)
    }

    pub fn with_id(id: uuid::Uuid, node_type: NodeType, properties: serde_json::Value) -> Self {
        Node {
            id,
            node_type,
            properties,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: uuid::Uuid,
    pub from_node_id: uuid::Uuid,
    pub to_node_id: uuid::Uuid,
    pub rel_type: RelType,
    pub properties: serde_json::Value,
}

impl Relationship {
    pub fn new(from_node_id: uuid::Uuid, to_node_id: uuid::Uuid, rel_type: RelType) -> Self {
        Self::with_id(uuid::Uuid::new_v4(), from_node_id, to_node_id, rel_type)
    }

    pub fn with_id(
        id: uuid::Uuid,
        from_node_id: uuid::Uuid,
        to_node_id: uuid::Uuid,
        rel_type: RelType,
    ) -> Self {
        Relationship {
            id,
            from_node_id,
            to_node_id,
            rel_type,
            properties: serde_json::json!({}),
        }
    }
}
