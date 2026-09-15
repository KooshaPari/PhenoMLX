//! `lazy_populate` cache behavior — verify that the second call hits
//! the gate and never re-invokes the adapter.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use hwledger_search_core::{CandidateId, CoreError, RawModel, SourceAdapter};
use hwledger_search_ingest::{lazy_populate, PopulateGate};

#[derive(Clone)]
pub struct FakeAdapter {
    canned: RawModel,
    fetch_count: Arc<AtomicUsize>,
}

impl FakeAdapter {
    fn new(canned: RawModel) -> Self {
        Self {
            canned,
            fetch_count: Arc::new(AtomicUsize::new(0)),
        }
    }
    fn fetch_count(&self) -> usize {
        self.fetch_count.load(Ordering::SeqCst)
    }
}

impl SourceAdapter for FakeAdapter {
    fn name(&self) -> &str {
        "fake"
    }
    fn list_candidates(&self, _query: Option<&str>, _limit: usize) -> Vec<CandidateId> {
        Vec::new()
    }
    fn fetch_raw(&self, id: &CandidateId) -> Result<RawModel, CoreError> {
        self.fetch_count.fetch_add(1, Ordering::SeqCst);
        assert_eq!(id.source, "fake");
        Ok(self.canned.clone())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn lazy_populate_caches_after_first_fetch() {
    let adapter = FakeAdapter::new(RawModel::new("fake/x", "fake"));
    let gate = PopulateGate::default();

    // First call: must hit the adapter.
    let first = lazy_populate(&adapter.clone(), &gate, "fake/x")
        .await
        .expect("first fetch ok");
    assert_eq!(first.id, "fake/x");
    assert_eq!(adapter.fetch_count(), 1);
    assert_eq!(gate.len(), 1);

    // Second call: must hit the cache — fetch_count unchanged.
    let second = lazy_populate(&adapter.clone(), &gate, "fake/x")
        .await
        .expect("cache hit ok");
    assert_eq!(second.id, "fake/x");
    assert_eq!(adapter.fetch_count(), 1, "second call must not re-fetch");
    assert_eq!(gate.len(), 1, "gate must still hold exactly one entry");
}

#[tokio::test(flavor = "current_thread")]
async fn lazy_populate_warms_two_distinct_ids() {
    let adapter = FakeAdapter::new(RawModel::new("fake/y", "fake"));
    let gate = PopulateGate::default();

    let _ = lazy_populate(&adapter.clone(), &gate, "fake/y").await.unwrap();
    assert_eq!(gate.len(), 1);

    // Same id, same adapter name → cache hit, no second fetch.
    let _ = lazy_populate(&adapter.clone(), &gate, "fake/y").await.unwrap();
    assert_eq!(gate.len(), 1);
    assert_eq!(adapter.fetch_count(), 1);
}
