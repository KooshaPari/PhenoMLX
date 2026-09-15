//! `build_seed_index` end-to-end coverage using a canned fake adapter —
//! no network calls, no real tantivy store.

use std::sync::atomic::{AtomicUsize, Ordering};

use hwledger_search_core::{CandidateId, CoreError, RawModel, SourceAdapter};
use hwledger_search_ingest::{build_seed_index, SeedBuild, SeedReport, SeedSink};

/// Trivial seed sink that just counts upserts.
struct CountingSink {
    upserts: AtomicUsize,
}

impl CountingSink {
    fn new() -> Self {
        Self {
            upserts: AtomicUsize::new(0),
        }
    }
    fn count(&self) -> usize {
        self.upserts.load(Ordering::SeqCst)
    }
}

impl SeedSink for CountingSink {
    fn upsert(&mut self, _raw: &RawModel) -> Result<(), String> {
        self.upserts.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Canned adapter used to inject a fixed set of `RawModel`s into the
/// seed builder without touching the network.
pub struct FakeAdapter {
    canned: Vec<RawModel>,
}

impl FakeAdapter {
    fn new(canned: Vec<RawModel>) -> Self {
        Self { canned }
    }
}

impl SourceAdapter for FakeAdapter {
    fn name(&self) -> &str {
        "fake"
    }
    fn list_candidates(&self, _query: Option<&str>, limit: usize) -> Vec<CandidateId> {
        self.canned
            .iter()
            .take(limit)
            .map(|m| CandidateId::new("fake", m.id.clone()))
            .collect()
    }
    fn fetch_raw(&self, id: &CandidateId) -> Result<RawModel, CoreError> {
        self.canned
            .iter()
            .find(|m| m.id == id.id)
            .cloned()
            .ok_or_else(|| CoreError::not_found(id.id.clone()))
    }
}

fn canned(n: usize) -> Vec<RawModel> {
    (0..n)
        .map(|i| {
            let mut m = RawModel::new(format!("fake/{i}"), "fake".to_string());
            m.downloads = Some(i as u64);
            m
        })
        .collect()
}

#[test]
fn build_seed_index_indexes_all_canned_models() {
    let adapter = FakeAdapter::new(canned(5));
    let mut sink = CountingSink::new();
    let build = SeedBuild {
        queries: vec!["anything".to_string()],
        size: 16,
    };
    let report: SeedReport = build_seed_index(&adapter, &mut sink, &build);
    assert_eq!(report.models_indexed, 5);
    assert_eq!(report.errors, 0);
    assert_eq!(report.queries_run, 1);
    assert_eq!(sink.count(), 5);
}
