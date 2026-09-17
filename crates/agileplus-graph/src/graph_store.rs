// SPDX-License-Identifier: MIT OR Apache-2.0
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

use super::types::{Node, NodeType, RelType, Relationship};

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Query error: {0}")]
    QueryError(String),
    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn upsert_node(&self, node: &Node) -> Result<(), GraphError>;
    async fn create_relationship(&self, relationship: &Relationship) -> Result<(), GraphError>;
    async fn delete_relationship(&self, relationship_id: uuid::Uuid) -> Result<(), GraphError>;
    async fn get_dependencies(&self, node_id: uuid::Uuid) -> Result<Vec<uuid::Uuid>, GraphError>;
    async fn get_blocking_path(&self, node_id: uuid::Uuid) -> Result<Vec<uuid::Uuid>, GraphError>;
    async fn health_check(&self) -> Result<(), GraphError>;
}

pub struct InMemoryGraphStore {
    nodes: Mutex<HashMap<uuid::Uuid, Node>>,
    relationships: Mutex<Vec<Relationship>>,
}

impl InMemoryGraphStore {
    pub fn new() -> Self {
        InMemoryGraphStore {
            nodes: Mutex::new(HashMap::new()),
            relationships: Mutex::new(Vec::new()),
        }
    }

    pub fn get_node(&self, node_id: uuid::Uuid) -> Option<Node> {
        self.nodes.lock().unwrap().get(&node_id).cloned()
    }

    pub fn get_nodes_by_type(&self, node_type: NodeType) -> Vec<Node> {
        self.nodes
            .lock()
            .unwrap()
            .values()
            .filter(|n| n.node_type == node_type)
            .cloned()
            .collect()
    }

    pub fn get_relationships_from(&self, node_id: uuid::Uuid) -> Vec<Relationship> {
        self.relationships
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.from_node_id == node_id)
            .cloned()
            .collect()
    }

    pub fn get_relationships_to(&self, node_id: uuid::Uuid) -> Vec<Relationship> {
        self.relationships
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.to_node_id == node_id)
            .cloned()
            .collect()
    }
}

impl Default for InMemoryGraphStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GraphStore for InMemoryGraphStore {
    async fn upsert_node(&self, node: &Node) -> Result<(), GraphError> {
        let mut nodes = self.nodes.lock().unwrap();
        nodes.insert(node.id, node.clone());
        Ok(())
    }

    async fn create_relationship(&self, relationship: &Relationship) -> Result<(), GraphError> {
        let mut rels = self.relationships.lock().unwrap();
        rels.push(relationship.clone());
        Ok(())
    }

    async fn delete_relationship(&self, relationship_id: uuid::Uuid) -> Result<(), GraphError> {
        let mut rels = self.relationships.lock().unwrap();
        rels.retain(|r| r.id != relationship_id);
        Ok(())
    }

    async fn get_dependencies(&self, node_id: uuid::Uuid) -> Result<Vec<uuid::Uuid>, GraphError> {
        let rels = self.relationships.lock().unwrap();
        let deps: Vec<uuid::Uuid> = rels
            .iter()
            .filter(|r| r.from_node_id == node_id && r.rel_type == RelType::DependsOn)
            .map(|r| r.to_node_id)
            .collect();
        Ok(deps)
    }

    async fn get_blocking_path(&self, node_id: uuid::Uuid) -> Result<Vec<uuid::Uuid>, GraphError> {
        let rels = self.relationships.lock().unwrap();
        let blockers: Vec<uuid::Uuid> = rels
            .iter()
            .filter(|r| r.to_node_id == node_id && r.rel_type == RelType::Blocks)
            .map(|r| r.from_node_id)
            .collect();
        Ok(blockers)
    }

    async fn health_check(&self) -> Result<(), GraphError> {
        let _guard = self
            .nodes
            .lock()
            .map_err(|e| GraphError::ConnectionError(format!("node store poisoned: {e}")))?;
        let _guard = self
            .relationships
            .lock()
            .map_err(|e| GraphError::ConnectionError(format!("relationship store poisoned: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_upsert_node() {
        let store = InMemoryGraphStore::new();
        let node_id = uuid::Uuid::new_v4();
        let node = Node::with_id(
            node_id,
            NodeType::Feature,
            serde_json::json!({"slug": "feat-1"}),
        );

        store.upsert_node(&node).await.unwrap();

        assert_eq!(node.id, node_id);
        assert_eq!(
            store.get_node(node.id).unwrap().properties["slug"],
            "feat-1"
        );
    }

    #[tokio::test]
    async fn test_create_and_delete_relationship() {
        let store = InMemoryGraphStore::new();
        let node1 = Node::new(NodeType::Feature, serde_json::json!({"slug": "f1"}));
        let node2 = Node::new(NodeType::Feature, serde_json::json!({"slug": "f2"}));

        store.upsert_node(&node1).await.unwrap();
        store.upsert_node(&node2).await.unwrap();

        let rel = Relationship::new(node1.id, node2.id, RelType::DependsOn);
        store.create_relationship(&rel).await.unwrap();

        let deps = store.get_dependencies(node1.id).await.unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], node2.id);

        store.delete_relationship(rel.id).await.unwrap();
        let deps = store.get_dependencies(node1.id).await.unwrap();
        assert!(deps.is_empty());
    }

    #[tokio::test]
    async fn test_relationship_with_stable_uuid() {
        let store = InMemoryGraphStore::new();
        let node1 = Node::new(NodeType::Feature, serde_json::json!({"slug": "f1"}));
        let node2 = Node::new(NodeType::Feature, serde_json::json!({"slug": "f2"}));
        let relationship_id = uuid::Uuid::new_v4();

        store.upsert_node(&node1).await.unwrap();
        store.upsert_node(&node2).await.unwrap();

        let rel = Relationship::with_id(relationship_id, node1.id, node2.id, RelType::DependsOn);
        store.create_relationship(&rel).await.unwrap();

        let rels = store.get_relationships_from(node1.id);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].id, relationship_id);
    }

    #[tokio::test]
    async fn test_get_blocking_path() {
        let store = InMemoryGraphStore::new();
        let blocker = Node::new(
            NodeType::WorkPackage,
            serde_json::json!({"title": "blocker"}),
        );
        let blocked = Node::new(
            NodeType::WorkPackage,
            serde_json::json!({"title": "blocked"}),
        );

        store.upsert_node(&blocker).await.unwrap();
        store.upsert_node(&blocked).await.unwrap();

        let rel = Relationship::new(blocker.id, blocked.id, RelType::Blocks);
        store.create_relationship(&rel).await.unwrap();

        let blockers = store.get_blocking_path(blocked.id).await.unwrap();
        assert_eq!(blockers.len(), 1);
        assert_eq!(blockers[0], blocker.id);
    }

    #[tokio::test]
    async fn test_health_check() {
        let store = InMemoryGraphStore::new();
        assert!(store.health_check().await.is_ok());
    }
}
