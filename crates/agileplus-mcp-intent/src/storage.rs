// SPDX-License-Identifier: MIT OR Apache-2.0
//! Optional storage adapter for persisting intent graphs into the AgilePlus SQLite database.

use anyhow::Context;
use serde_json::Value;

use agileplus_domain::domain::feature::Feature;
use agileplus_domain::ports::storage::StoragePort;
use agileplus_sqlite::SqliteStorageAdapter;

use crate::types::IntentGraph;

/// Store an intent graph into the AgilePlus database.
///
/// Maps:
/// - `Intent` nodes → ignored (no direct table)
/// - `Plan` nodes → ignored (no direct table)
/// - `Feature` nodes → `features` table
/// - `Story` nodes → `stories` table (requires epic + project)
///
/// For simplicity, only features are stored directly.  Stories require
/// an epic which requires a project; these are created on-demand.
pub async fn store_graph(
    db: &SqliteStorageAdapter,
    graph: &IntentGraph,
) -> anyhow::Result<Vec<i64>> {
    let mut ids = vec![];

    for node in &graph.nodes {
        match node.node_type {
            crate::types::NodeType::Feature => {
                let slug = node.id.split('#').nth(1).unwrap_or("unknown");
                let friendly_name = node.title.clone();
                let mut feature = Feature::new(slug, &friendly_name, [0u8; 32], None);
                // Add metadata as labels
                if let Some(Value::Object(props)) = &node.properties {
                    if let Some(Value::Array(tags)) = props.get("tags") {
                        let tag_strs: Vec<String> = tags
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        feature.labels.extend(tag_strs);
                    }
                }
                let id = StoragePort::create_feature(db, &feature)
                    .await
                    .with_context(|| format!("store feature {}", node.id))?;
                ids.push(id);
                tracing::info!("stored feature id={id} from node {}", node.id);
            }
            crate::types::NodeType::Story => {
                // Stories require an epic and project.  Skip for now unless we create a default project.
                tracing::debug!("skipping story node {} (requires epic+project)", node.id);
            }
            _ => {
                tracing::debug!("skipping node {} (type {:?})", node.id, node.node_type);
            }
        }
    }

    Ok(ids)
}

/// Open a SQLite storage adapter from the `AGILEPLUS_DB` env or default path.
pub fn open_storage() -> anyhow::Result<SqliteStorageAdapter> {
    let db_path = std::env::var("AGILEPLUS_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("agileplus.db"));
    SqliteStorageAdapter::new(&db_path)
        .with_context(|| format!("open SQLite storage at {db_path:?}"))
}

/// Store a graph and return a summary.
pub async fn store_and_summarize(
    db: &SqliteStorageAdapter,
    graph: &IntentGraph,
) -> anyhow::Result<StorageSummary> {
    let ids = store_graph(db, graph).await?;
    Ok(StorageSummary {
        features_stored: ids.len(),
        ids,
    })
}

/// Summary of what was stored.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageSummary {
    pub features_stored: usize,
    pub ids: Vec<i64>,
}
