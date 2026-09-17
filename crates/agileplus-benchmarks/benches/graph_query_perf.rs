//! T120 – Graph query performance benchmark.
//!
//! Benchmarks the in-memory `InMemoryGraphStore` to measure the overhead of:
//! - Node creation (Feature, WorkPackage, Agent)
//! - Node lookup by ID
//! - Dependency / blocking-path traversal queries
//! - Bulk node creation (seeding 100 features)
//!
//! Note: The in-memory backend provides a simple HashMap-based storage.
//! A Neo4j-backed benchmark would be added in CI using the `neo4j` feature.

use agileplus_graph::{GraphStore, InMemoryGraphStore, Node, NodeType, RelType, Relationship};
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use tokio::runtime::Runtime;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_store() -> InMemoryGraphStore {
    InMemoryGraphStore::new()
}

// ---------------------------------------------------------------------------
// Benchmark: create a single feature node
// ---------------------------------------------------------------------------

fn bench_create_feature_node(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("graph_create_feature_node", |b| {
        b.iter(|| {
            let store = make_store();
            rt.block_on(async {
                let node = Node::new(
                    NodeType::Feature,
                    serde_json::json!({
                        "slug": "feat-bench",
                        "state": "Created",
                        "friendly_name": "Bench Feature"
                    }),
                );
                store.upsert_node(&node).await.expect("create");
                black_box(node.id)
            });
        });
    });
}

// ---------------------------------------------------------------------------
// Benchmark: get feature node by ID (via traversal)
// ---------------------------------------------------------------------------

fn bench_get_feature_node(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let store = make_store();
    let feature_ids: Vec<Uuid> = rt.block_on(async {
        let mut ids = Vec::new();
        for i in 1..=100_u64 {
            let node = Node::new(
                NodeType::Feature,
                serde_json::json!({
                    "id": i,
                    "slug": format!("feat-{}", i),
                    "state": "Created",
                    "friendly_name": format!("Feature {}", i)
                }),
            );
            store.upsert_node(&node).await.unwrap();
            ids.push(node.id);
        }
        ids
    });

    c.bench_function("graph_get_feature_node", |b| {
        b.iter(|| {
            let id = black_box(feature_ids[49]); // index 49 = feature id 50
            let node = store.get_node(id);
            black_box(node.map(|n| n.properties["id"].as_i64()))
        });
    });
}

// ---------------------------------------------------------------------------
// Benchmark: seed N nodes (measures bulk-creation throughput)
// ---------------------------------------------------------------------------

fn bench_seed_n_features(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("graph_seed_features");

    for count in [10_u64, 50, 100] {
        group.bench_with_input(BenchmarkId::new("seed_features", count), &count, |b, &n| {
            b.iter(|| {
                let store = make_store();
                rt.block_on(async {
                    for i in 1..=n {
                        let node = Node::new(
                            NodeType::Feature,
                            serde_json::json!({
                                "id": i,
                                "slug": format!("feat-{}", i),
                                "state": "Created",
                                "friendly_name": format!("Feature {}", i)
                            }),
                        );
                        store.upsert_node(&node).await.expect("create");
                    }
                });
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: relationship creation
// ---------------------------------------------------------------------------

fn bench_create_relationships(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let store = make_store();
    rt.block_on(async {
        for i in 1..=10_u64 {
            let feature = Node::new(
                NodeType::Feature,
                serde_json::json!({"id": i, "slug": format!("f-{}", i)}),
            );
            let wp = Node::new(
                NodeType::WorkPackage,
                serde_json::json!({"id": i, "title": format!("WP-{}", i)}),
            );
            store.upsert_node(&feature).await.unwrap();
            store.upsert_node(&wp).await.unwrap();
        }
    });

    c.bench_function("graph_create_owns_relationship", |b| {
        b.iter(|| {
            let store = make_store();
            rt.block_on(async {
                let feature = Node::new(
                    NodeType::Feature,
                    serde_json::json!({"id": 1, "slug": "f-1"}),
                );
                let wp = Node::new(
                    NodeType::WorkPackage,
                    serde_json::json!({"id": 1, "title": "WP-1"}),
                );
                store.upsert_node(&feature).await.unwrap();
                store.upsert_node(&wp).await.unwrap();
                let rel = Relationship::new(feature.id, wp.id, RelType::Owns);
                store.create_relationship(&rel).await.expect("owns");
            });
        });
    });
}

// ---------------------------------------------------------------------------
// Benchmark: dependency-chain traversal query
// ---------------------------------------------------------------------------

fn bench_dependency_chain_query(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let store = make_store();
    rt.block_on(async {
        for i in 1..=100_u64 {
            let node = Node::new(
                NodeType::Feature,
                serde_json::json!({
                    "id": i,
                    "slug": format!("feat-{}", i),
                    "state": "Created",
                    "friendly_name": format!("Feature {}", i)
                }),
            );
            store.upsert_node(&node).await.unwrap();
        }
    });

    c.bench_function("graph_dependency_chain_query", |b| {
        b.iter(|| {
            let store = &store;
            rt.block_on(async {
                let deps = store
                    .get_dependencies(black_box(Uuid::nil()))
                    .await
                    .expect("query");
                black_box(deps.len())
            })
        });
    });
}

criterion_group!(
    benches,
    bench_create_feature_node,
    bench_get_feature_node,
    bench_seed_n_features,
    bench_create_relationships,
    bench_dependency_chain_query,
);
criterion_main!(benches);

// ---------------------------------------------------------------------------
// Smoke tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn create_and_get_feature_smoke() {
        let store = InMemoryGraphStore::new();
        let node = Node::new(
            NodeType::Feature,
            serde_json::json!({"slug": "smoke", "state": "Created", "friendly_name": "Smoke"}),
        );
        store.upsert_node(&node).await.unwrap();
        let retrieved = store.get_node(node.id);
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn seed_100_features_smoke() {
        let store = InMemoryGraphStore::new();
        for i in 1..=100_u64 {
            let node = Node::new(
                NodeType::Feature,
                serde_json::json!({
                    "id": i,
                    "slug": format!("feat-{}", i),
                    "state": "Created",
                    "friendly_name": format!("Feature {}", i)
                }),
            );
            store.upsert_node(&node).await.unwrap();
        }
        let nodes = store.get_nodes_by_type(NodeType::Feature);
        assert_eq!(nodes.len(), 100);
    }

    #[tokio::test]
    async fn dependency_chain_empty_smoke() {
        let store = InMemoryGraphStore::new();
        let node = Node::new(
            NodeType::Feature,
            serde_json::json!({"slug": "test", "state": "Created"}),
        );
        store.upsert_node(&node).await.unwrap();
        let deps = store.get_dependencies(node.id).await.unwrap();
        assert!(deps.is_empty());
    }

    #[tokio::test]
    async fn relationship_create_smoke() {
        let store = InMemoryGraphStore::new();
        let feature = Node::new(
            NodeType::Feature,
            serde_json::json!({"slug": "f1", "state": "Created"}),
        );
        let wp = Node::new(
            NodeType::WorkPackage,
            serde_json::json!({"title": "WP1", "ordinal": 1}),
        );
        store.upsert_node(&feature).await.unwrap();
        store.upsert_node(&wp).await.unwrap();
        let rel = Relationship::new(feature.id, wp.id, RelType::Owns);
        store.create_relationship(&rel).await.unwrap();
        let rels = store.get_relationships_from(feature.id);
        assert_eq!(rels.len(), 1);
    }
}
