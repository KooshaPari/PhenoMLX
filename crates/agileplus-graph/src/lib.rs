// SPDX-License-Identifier: MIT OR Apache-2.0
pub mod graph_store;
pub mod neo4j_store;
pub mod types;

pub use graph_store::{GraphError, GraphStore, InMemoryGraphStore};
#[cfg(feature = "neo4j")]
pub use neo4j_store::Neo4jGraphStore;
pub use types::{Node, NodeType, RelType, Relationship};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Graph error: {0}")]
    Graph(#[from] GraphError),
    #[error("Config error: {0}")]
    Config(String),
}
