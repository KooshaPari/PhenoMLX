//! Neo4j-backed graph store — enabled by the `neo4j` feature flag.
//! Uses `neo4rs` to execute Cypher queries against a running Neo4j instance.

#[cfg(feature = "neo4j")]
mod inner {
    use std::sync::Arc;

    use async_trait::async_trait;
    use neo4rs::{query, Graph as Neo4jGraph};
    use uuid::Uuid;

    use crate::{
        graph_store::{GraphError, GraphStore},
        types::{Node, Relationship},
    };

    pub struct Neo4jGraphStore {
        graph: Arc<Neo4jGraph>,
    }

    impl Neo4jGraphStore {
        pub async fn connect(uri: &str, user: &str, password: &str) -> Result<Self, GraphError> {
            let graph = Neo4jGraph::new(uri, user, password)
                .await
                .map_err(|e| GraphError::ConnectionError(e.to_string()))?;
            Ok(Self {
                graph: Arc::new(graph),
            })
        }
    }

    #[async_trait]
    impl GraphStore for Neo4jGraphStore {
        async fn upsert_node(&self, node: &Node) -> Result<(), GraphError> {
            let kind = format!("{:?}", node.node_type);
            let props = node.properties.to_string();
            self.graph
                .run(
                    query(
                        "MERGE (n:Node {id: $id}) \
                         SET n.kind = $kind, n.properties = $props",
                    )
                    .param("id", node.id.to_string())
                    .param("kind", kind)
                    .param("props", props),
                )
                .await
                .map_err(|e| GraphError::QueryError(e.to_string()))
        }

        async fn create_relationship(&self, rel: &Relationship) -> Result<(), GraphError> {
            let kind = format!("{:?}", rel.rel_type);
            self.graph
                .run(
                    query(
                        "MATCH (a:Node {id: $from}), (b:Node {id: $to}) \
                         MERGE (a)-[r:LINK {id: $rid, kind: $kind}]->(b)",
                    )
                    .param("from", rel.from_node_id.to_string())
                    .param("to", rel.to_node_id.to_string())
                    .param("rid", rel.id.to_string())
                    .param("kind", kind),
                )
                .await
                .map_err(|e| GraphError::QueryError(e.to_string()))
        }

        async fn delete_relationship(&self, relationship_id: Uuid) -> Result<(), GraphError> {
            self.graph
                .run(
                    query("MATCH ()-[r:LINK {id: $id}]->() DELETE r")
                        .param("id", relationship_id.to_string()),
                )
                .await
                .map_err(|e| GraphError::QueryError(e.to_string()))
        }

        async fn get_dependencies(&self, node_id: Uuid) -> Result<Vec<Uuid>, GraphError> {
            let mut result = self
                .graph
                .execute(
                    query(
                        "MATCH (n:Node {id: $id})-[:LINK]->(dep:Node) \
                         RETURN dep.id AS dep_id",
                    )
                    .param("id", node_id.to_string()),
                )
                .await
                .map_err(|e| GraphError::QueryError(e.to_string()))?;

            let mut ids = Vec::new();
            while let Ok(Some(row)) = result.next().await {
                if let Ok(id_str) = row.get::<String>("dep_id")
                    && let Ok(uid) = Uuid::parse_str(&id_str)
                {
                    ids.push(uid);
                }
            }
            Ok(ids)
        }

        async fn get_blocking_path(&self, node_id: Uuid) -> Result<Vec<Uuid>, GraphError> {
            // Shortest path from node to any node it transitively blocks.
            let mut result = self
                .graph
                .execute(
                    query(
                        "MATCH path = (n:Node {id: $id})-[:LINK*1..10]->(blocker:Node) \
                         RETURN blocker.id AS blocker_id",
                    )
                    .param("id", node_id.to_string()),
                )
                .await
                .map_err(|e| GraphError::QueryError(e.to_string()))?;

            let mut ids = Vec::new();
            while let Ok(Some(row)) = result.next().await {
                if let Ok(id_str) = row.get::<String>("blocker_id")
                    && let Ok(uid) = Uuid::parse_str(&id_str)
                {
                    ids.push(uid);
                }
            }
            Ok(ids)
        }

        async fn health_check(&self) -> Result<(), GraphError> {
            self.graph
                .run(query("RETURN 1"))
                .await
                .map_err(|e| GraphError::QueryError(e.to_string()))
        }
    }
}

#[cfg(feature = "neo4j")]
pub use inner::Neo4jGraphStore;
