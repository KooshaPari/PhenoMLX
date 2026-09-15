//! Seed-index builder.
//!
//! Given a list of search queries and a [`SourceAdapter`] (typically a
//! [`crate::HuggingFaceAdapter`]), iterate the queries, fan out per-query
//! candidate lists, fetch their raw payloads, and push parsed models into
//! a [`SeedSink`] (the tantivy index in production, a `FakeSink` in tests).
//!
//! The builder is intentionally stateless and sync; the underlying
//! adapter is responsible for the actual HTTP I/O. When run against a
//! live HF Hub the call sites will hit the network — tests must inject
//! a fake adapter instead (see `tests/seed_size.rs`).

use hwledger_search_core::{RawModel, SourceAdapter};

use crate::huggingface::HuggingFaceAdapter;

/// Default HF search queries used when a [`SeedBuild`] is built with
/// `Default::default()`. Covers the model families we care about for
/// fleet capacity planning.
pub const DEFAULT_SEED_QUERIES: &[&str] = &[
    "qwen2.5",
    "llama-3.1",
    "deepseek-v3",
    "gemma-2",
    "mistral-nemo",
    "phi-3",
    "codestral",
    "bge-large",
];

/// Inputs to a seed-index build.
#[derive(Debug, Clone)]
pub struct SeedBuild {
    /// HuggingFace search queries to fan out across.
    pub queries: Vec<String>,
    /// Soft cap on per-query candidates.
    pub size: usize,
}

impl Default for SeedBuild {
    fn default() -> Self {
        Self {
            queries: DEFAULT_SEED_QUERIES.iter().map(|s| s.to_string()).collect(),
            size: 2000,
        }
    }
}

/// Summary of a single seed-index build run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeedReport {
    /// Number of models successfully upserted into the sink.
    pub models_indexed: u32,
    /// Number of per-candidate failures (fetch / parse / upsert).
    pub errors: u32,
    /// Number of queries that were actually dispatched.
    pub queries_run: u32,
}

/// Anything that can accept a parsed [`RawModel`] for indexing.
///
/// `TantivyStore` (in `hwledger-search-index`) will implement this trait
/// once that crate is filled in. Tests provide their own `FakeSink`.
pub trait SeedSink {
    /// Insert or replace the model under its canonical key.
    /// Implementations report failures as `String` so the seed builder
    /// never needs to know about a sink-specific error type.
    fn upsert(&mut self, raw: &RawModel) -> Result<(), String>;
}

/// Iterate the configured queries, fetch candidate lists, and push each
/// parsed model into `sink`. Returns a [`SeedReport`] summarizing the run.
///
/// Per-query allocation is `build.size / N`, rounded up, with a floor of
/// `1` so empty query lists still produce a sensible (if zero) run.
pub fn build_seed_index<A>(
    adapter: &A,
    sink: &mut dyn SeedSink,
    build: &SeedBuild,
) -> SeedReport
where
    A: SourceAdapter + ?Sized,
{
    let mut report = SeedReport::default();
    let n = build.queries.len().max(1);
    let per_query = (build.size / n).max(1);

    for q in &build.queries {
        report.queries_run += 1;
        let candidates = adapter.list_candidates(Some(q.as_str()), per_query);
        for cid in candidates {
            match adapter.fetch_raw(&cid) {
                Ok(raw) => match sink.upsert(&raw) {
                    Ok(()) => report.models_indexed += 1,
                    Err(e) => {
                        tracing::warn!(model = %cid, error = %e, "seed upsert failed");
                        report.errors += 1;
                    }
                },
                Err(e) => {
                    tracing::warn!(model = %cid, error = %e, "seed fetch failed");
                    report.errors += 1;
                }
            }
        }
    }

    report
}

/// Convenience wrapper that restricts the generic to the concrete
/// `HuggingFaceAdapter`. Retained for callers that want a non-trait
/// entry point and matches the spec's public surface.
#[allow(dead_code)]
pub fn build_seed_index_hf(
    adapter: &HuggingFaceAdapter,
    sink: &mut dyn SeedSink,
    build: &SeedBuild,
) -> SeedReport {
    build_seed_index(adapter, sink, build)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_seed_build_has_nonempty_queries() {
        let b = SeedBuild::default();
        assert!(!b.queries.is_empty());
        assert_eq!(b.size, 2000);
        assert!(b.queries.iter().any(|q| q == "qwen2.5"));
    }

    #[test]
    fn seed_report_default_is_zero() {
        let r = SeedReport::default();
        assert_eq!(r.models_indexed, 0);
        assert_eq!(r.errors, 0);
        assert_eq!(r.queries_run, 0);
    }
}
