//! Lazy, on-demand population of the raw-model cache.
//!
//! When a query mentions a model id that hasn't been seen before, we
//! fetch its full payload exactly once and stash it in a shared
//! [`PopulateGate`] so subsequent lookups (same id, same process) hit
//! the cache. The gate is intentionally sync (`std::sync::Mutex`) —
//! the critical section is short and we want to be able to share it
//! across both async and sync call sites without a separate async
//! primitive.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use hwledger_search_core::{CandidateId, RawModel, SourceAdapter};

use crate::error::IngestError;

/// Thread-safe, lazily-populated cache of [`RawModel`]s keyed by
/// `<source>::<id>` — the same scheme used by [`RawModel::key`].
#[derive(Debug, Default, Clone)]
pub struct PopulateGate {
    inner: Arc<Mutex<HashMap<String, RawModel>>>,
}

impl PopulateGate {
    /// Build an empty gate.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of cached models. Intended for tests / diagnostics.
    pub fn len(&self) -> usize {
        self.inner.lock().expect("populate gate poisoned").len()
    }

    /// Whether the gate has no cached models.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Look up a single id without inserting. Returns a borrowed clone
    /// of the cached model if present.
    pub fn get(&self, id: &str) -> Option<RawModel> {
        self.inner
            .lock()
            .expect("populate gate poisoned")
            .get(id)
            .cloned()
    }

    /// Insert a model into the gate unconditionally. Returns the
    /// previous value, if any.
    pub fn insert(&self, raw: RawModel) -> Option<RawModel> {
        self.inner
            .lock()
            .expect("populate gate poisoned")
            .insert(raw.key(), raw)
    }

    /// Look up by a [`CandidateId`] (composes `source::id`).
    pub fn get_candidate(&self, cid: &CandidateId) -> Option<RawModel> {
        self.get(&cid.key())
    }
}

/// Fetch and cache a model on first lookup, return the cached copy on
/// subsequent calls.
///
/// Concurrent callers serialize on the gate's mutex; only the first
/// misses the cache and reaches the adapter. Errors are propagated
/// without polling the cache — a failed fetch leaves the gate untouched
/// so the next caller can retry.
pub async fn lazy_populate<A>(
    adapter: &A,
    gate: &PopulateGate,
    id: &str,
) -> Result<RawModel, IngestError>
where
    A: SourceAdapter + Clone + Send + Sync + 'static,
{
    let cid = CandidateId::new(adapter.name(), id);

    // Fast path: cache hit.
    if let Some(cached) = gate.get_candidate(&cid) {
        return Ok(cached);
    }

    // Slow path: fetch, then insert.
    let raw = fetch_via_adapter(adapter, &cid).await?;

    let value = gate
        .inner
        .lock()
        .expect("populate gate poisoned")
        .entry(cid.key())
        .or_insert_with(|| raw.clone())
        .clone();
    Ok(value)
}

/// Tunnel the (sync) [`SourceAdapter::fetch_raw`] call into an async
/// context. Uses `spawn_blocking` so the worker thread isn't blocked
/// while waiting on the upstream HTTP round-trip.
async fn fetch_via_adapter<A>(adapter: &A, cid: &CandidateId) -> Result<RawModel, IngestError>
where
    A: SourceAdapter + Clone + Send + Sync + 'static,
{
    let adapter = adapter.clone();
    let cid = cid.clone();
    let raw = tokio::task::spawn_blocking(move || adapter.fetch_raw(&cid).map_err(core_to_ingest))
        .await
        .map_err(|e| IngestError::backend(format!("join error: {e}")))??;
    Ok(raw)
}

/// Lossy `CoreError → IngestError` conversion. The adapter layer never
/// re-translates: any backend-side failure is reported as
/// `IngestError::Backend` so the caller can still distinguish
/// transport problems from upstream semantics.
fn core_to_ingest(e: hwledger_search_core::CoreError) -> IngestError {
    match e {
        hwledger_search_core::CoreError::Json(j) => IngestError::Json(j),
        other => IngestError::backend(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_gate_has_zero_length() {
        let g = PopulateGate::default();
        assert!(g.is_empty());
        assert_eq!(g.len(), 0);
        assert!(g.get("anything").is_none());
    }
}
