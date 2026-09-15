//! Public hybrid search entry point.
//!
//! v1 (default features) is BM25-only: there is no vector index linked, so
//! [`run_hybrid`] is functionally equivalent to a post-filtered BM25 lookup.
//! When the `lancedb` cargo feature is enabled the function gains two extra
//! arguments (`Option<&LanceStore>` + `&[f32]`) and fuses BM25 + ANN with
//! RRF (`k = 60`) from [`hwledger_search_core::rrf_fuse`]. The OFF
//! signature is stable so callers don't have to refactor to take advantage
//! of the dense path when they adopt the feature.

use hwledger_search_core::{FusedResult, ModelKind, Query};
#[cfg(not(feature = "lancedb"))]
use hwledger_search_core::Facets;

use crate::error::IndexError;
use crate::tantivy_store::TantivyStore;

/// Run the hybrid search and return up to `k` results, sorted by score
/// descending.
///
/// v1 implementation (no `lancedb` feature):
///
/// 1. Asks Tantivy for `k * 2` BM25 hits on the free-text `q.text`.
/// 2. Filters by [`Facets::kinds`](hwledger_search_core::Facets::kinds) (if any). The v1 schema only wires up
///    `kind`, so the only facet dimension honored today is `kinds` — the
///    others (`modalities`, `arch_kinds`, `attention_kinds`, numeric
///    ranges, `license`, `provenance`, `quants`) are accepted but skipped
///    in this phase. We never *silently* drop a result that matches the
///    unstructured query, though — we simply don't filter it.
/// 3. Truncates to `k` rows.
/// 4. Wraps each row in a [`FusedResult`] with `score = bm25_score`.
///
/// `index.search` is internally sync, so this function is `async` only to
/// match the eventual hybrid (BM25 + LanceDB) signature — call it from
/// any async runtime and the body will simply resolve immediately.
#[cfg(not(feature = "lancedb"))]
pub async fn run_hybrid(
    index: &TantivyStore,
    q: &Query,
    k: usize,
) -> Result<Vec<FusedResult>, IndexError> {
    let bm25 = bm25_filtered(index, q, k)?;
    Ok(wrap_with_facets(bm25, &q.facets))
}

/// Run the hybrid search with optional dense ANN fusion and return up to
/// `k` results, sorted by RRF score descending.
///
/// When `lance.is_some()` *and* `query_vec` is non-empty, the BM25 hits
/// are fused with `LanceStore::ann(query_vec, k * 2)` via
/// [`hwledger_search_core::rrf_fuse`]. When `lance` is `None` (or the
/// `lancedb` feature is off) the function falls back to BM25-only — the
/// OFF signature in the cfg-gated sibling is identical so callers can
/// transition without API churn.
///
/// `q.facets.kinds` is honored on the BM25 side only; ANN hits whose id
/// doesn't appear in BM25 are still kept (so dense-only matches survive
/// the kinds filter, which is the point of having a dense index), but
/// they get a low base RRF score and end up lower in the final ranking.
#[cfg(feature = "lancedb")]
pub async fn run_hybrid(
    index: &TantivyStore,
    q: &Query,
    k: usize,
    lance: Option<&crate::lance_store::LanceStore>,
    query_vec: &[f32],
) -> Result<Vec<FusedResult>, IndexError> {
    let text = q.text.trim();
    if text.is_empty() || k == 0 {
        return Ok(Vec::new());
    }

    // Step 1 — pull a 2x over-fetch from BM25 so the post-filter step has
    // headroom even if `kinds` filters out some rows.
    let bm25 = bm25_with_score(index, text, k.saturating_mul(2).max(k))?;

    // Step 2 — apply `kinds` filter (only the kind facet is wired up today).
    let wanted_kinds: &[ModelKind] = &q.facets.kinds;
    let bm25_filtered: Vec<(String, f32)> = bm25
        .into_iter()
        .filter(|(id, _)| {
            if wanted_kinds.is_empty() {
                return true;
            }
            match index.kind_for_id(id) {
                Some(kind_str) => wanted_kinds
                    .iter()
                    .any(|w| w.to_string() == kind_str),
                None => false,
            }
        })
        .collect();

    // Step 3 — optional dense ANN. A missing LanceStore, a zero-dim query
    // vector, or any Lance-side error is tolerated by falling back to
    // BM25-only; the dense path is opt-in.
    let semantic: Vec<(String, f32)> = match (lance, query_vec.is_empty()) {
        (Some(store), false) => match store.ann(query_vec, k.saturating_mul(2).max(k)).await {
            Ok(ids) => ids
                .into_iter()
                // LanceDB returns hits in ascending-distance order; we
                // assign a synthetic score of `1.0 - i * eps` so that
                // earlier (closer) hits dominate the RRF combination.
                .enumerate()
                .map(|(i, id)| (id, 1.0 - (i as f32) * 1e-3))
                .collect(),
            Err(_) => Vec::new(),
        },
        _ => Vec::new(),
    };

    let fused = hwledger_search_core::rrf_fuse(&bm25_filtered, &semantic, k);

    let facets = q.facets.clone();
    Ok(fused
        .into_iter()
        // Compute the RRF score first (it borrows `s`) and only then move
        // `s.id` into `FusedResult::new`; otherwise the move + later borrow
        // would conflict.
        .map(|s| {
            let score = s.rrf_score(60);
            let id = s.id;
            FusedResult::new(id, score).with_facets(facets.clone())
        })
        .collect())
}

/// BM25-then-kinds-filter helper used by the OFF `run_hybrid` path.
/// Returns up to `k` `(id, bm25_score)` pairs.
///
/// Only compiled under the default-feature build; the ON path inlines
/// the same logic so the ANN fusion can share the same `Vec<(String,
/// f32)>` shape without a function-call boundary.
#[cfg(not(feature = "lancedb"))]
fn bm25_filtered(
    index: &TantivyStore,
    q: &Query,
    k: usize,
) -> Result<Vec<(String, f32)>, IndexError> {
    let text = q.text.trim();
    if text.is_empty() || k == 0 {
        return Ok(Vec::new());
    }
    let bm25 = bm25_with_score(index, text, k.saturating_mul(2).max(k))?;
    let wanted_kinds: &[ModelKind] = &q.facets.kinds;
    Ok(bm25
        .into_iter()
        .filter(|(id, _)| {
            if wanted_kinds.is_empty() {
                return true;
            }
            match index.kind_for_id(id) {
                Some(kind_str) => wanted_kinds
                    .iter()
                    .any(|w| w.to_string() == kind_str),
                None => false,
            }
        })
        .take(k)
        .collect())
}

/// Run a raw BM25 query and return up to `limit` `(id, score)` pairs.
///
/// Used by both the OFF and ON `run_hybrid` paths; always compiled so
/// neither cfg-gated entry point has to inline it.
fn bm25_with_score(
    index: &TantivyStore,
    text: &str,
    limit: usize,
) -> Result<Vec<(String, f32)>, IndexError> {
    let hits = index.search(text, limit)?;
    Ok(hits.into_iter().map(|h| (h.id, h.score)).collect())
}

/// Convert a list of `(id, score)` rows to `FusedResult`s, copying the
/// request's `Facets` onto each one so downstream consumers can see what
/// filters were active. Only compiled on the default-feature build.
#[cfg(not(feature = "lancedb"))]
fn wrap_with_facets(rows: Vec<(String, f32)>, facets: &Facets) -> Vec<FusedResult> {
    rows.into_iter()
        .map(|(id, score)| FusedResult::new(id, score).with_facets(facets.clone()))
        .collect()
}

/// Re-export [`IndexHit`] so callers only need `use hwledger_search_index::*`.
///
/// The type lives in the `tantivy_store` module alongside the store that
/// produces it; this re-export keeps the public crate surface flat.
pub use crate::tantivy_store::IndexHit;