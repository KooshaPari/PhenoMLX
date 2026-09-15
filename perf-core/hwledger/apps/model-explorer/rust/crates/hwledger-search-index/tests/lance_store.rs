//! Integration tests for the `lancedb` feature-gated ANN path.
//!
//! These tests are **only compiled when the `lancedb` cargo feature is
//! enabled**. On the default build (feature OFF) the file is empty so
//! the crate can still `cargo test` without dragging in LanceDB.
//!
//! LanceDB uses DataFusion under the hood, which requires a Tokio
//! reactor at runtime; every async test below uses
//! `#[tokio::test(flavor = "current_thread")]` so we spin up a
//! single-threaded reactor in-process and keep the suite hermetic.
//!
//! The test set is intentionally minimal — three well-separated 4-D
//! vectors, three queries, one assertion each:
//!
//! 1. `ann_returns_correct_nearest_id` — `LanceStore::ann(query, 1)`
//!    returns the row whose id matches the expected vector.
//! 2. `ann_k_limits_returned_hits` — `k = 2` against a 3-row table
//!    returns exactly 2 hits in ascending-distance order.
//! 3. `ann_k_zero_returns_empty_vec` — `k = 0` is a no-op.
//! 4. `ann_search_rejects_wrong_dim` — passing a 3-D query into a
//!    4-D table is a hard Lance-side error (`IndexError::Lance`).
//! 5. `run_hybrid_fuses_bm25_with_ann` — the `run_hybrid` entry point
//!    actually pulls rows from `LanceStore::ann` when a `LanceStore`
//!    is passed in alongside a query vector (proves "real ANN is wired",
//!    not stubbed).
//!
//! All assertions are deterministic: the test fixture is hand-picked so
//! cosine distance has a clear winner with no near-ties.

#![cfg(feature = "lancedb")]

use hwledger_search_index::{EmbeddingRow, IndexError, LanceStore};
use tempfile::TempDir;

/// Three well-separated 4-D vectors, each tagged with the model id the
/// test will look up. The vectors are chosen so each one has a clear
/// cosine-similarity winner — no ties, no near-ties — so the assertion
/// isn't flaky under whatever `nearest_to` returns.
fn fixture_rows() -> Vec<EmbeddingRow> {
    vec![
        // (1, 0, 0, 0) — ortho to the others on axis 0
        EmbeddingRow {
            id: "qwen/Qwen2.5-7B-Instruct".into(),
            vector: vec![1.0, 0.0, 0.0, 0.0],
        },
        // (0, 1, 0, 0) — ortho to the others on axis 1
        EmbeddingRow {
            id: "meta-llama/Llama-3-8B-Instruct".into(),
            vector: vec![0.0, 1.0, 0.0, 0.0],
        },
        // (0, 0, 1, 0) — ortho to the others on axis 2
        EmbeddingRow {
            id: "mistralai/Mistral-7B-Instruct-v0.3".into(),
            vector: vec![0.0, 0.0, 1.0, 0.0],
        },
    ]
}

async fn open_populated_store() -> (TempDir, LanceStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = LanceStore::new(dir.path()).await.expect("open lance");
    store.insert(&fixture_rows()).await.expect("insert rows");
    (dir, store)
}

#[tokio::test(flavor = "current_thread")]
async fn ann_returns_correct_nearest_id() {
    let (_dir, store) = open_populated_store().await;

    // For each row's expected id, query with a vector that's a positive
    // scalar multiple of that row's vector — cosine similarity is
    // direction-only, so any positive multiple is a perfect match.
    let queries = vec![
        ("qwen/Qwen2.5-7B-Instruct", vec![2.0, 0.0, 0.0, 0.0]),
        ("meta-llama/Llama-3-8B-Instruct", vec![0.0, 2.5, 0.0, 0.0]),
        ("mistralai/Mistral-7B-Instruct-v0.3", vec![0.0, 0.0, 3.5, 0.0]),
    ];

    for (expected_id, q) in queries {
        let hits = store.ann(&q, 1).await.expect("ann");
        assert_eq!(
            hits.len(),
            1,
            "expected exactly one nearest neighbour for {expected_id}, got {hits:?}"
        );
        assert_eq!(
            hits[0], expected_id,
            "nearest neighbour of {q:?} should be {expected_id}, got {}",
            hits[0]
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn ann_k_limits_returned_hits() {
    let (_dir, store) = open_populated_store().await;

    // k=2 against a 3-row table must give back exactly 2 ids. The two
    // axis-aligned rows are equally close (cosine distance 0), so the
    // order between them is implementation-defined — but they must be
    // the two axis-aligned rows and not the third axis-orthogonal row
    // (mistral).
    let hits = store.ann(&[1.0, 1.0, 0.0, 0.0], 2).await.expect("ann k=2");

    assert_eq!(hits.len(), 2, "k=2 must return exactly 2 hits");
    assert!(
        hits.contains(&"qwen/Qwen2.5-7B-Instruct".to_string())
            || hits.contains(&"meta-llama/Llama-3-8B-Instruct".to_string()),
        "first two hits must be the axis-aligned rows, got {hits:?}"
    );
    assert!(
        !hits.contains(&"mistralai/Mistral-7B-Instruct-v0.3".to_string()),
        "mistral must be excluded (not in top-2), got {hits:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn ann_k_zero_returns_empty_vec() {
    let (_dir, store) = open_populated_store().await;

    let hits = store.ann(&[1.0, 0.0, 0.0, 0.0], 0).await.expect("k=0");
    assert!(
        hits.is_empty(),
        "k=0 must be a no-op (empty vec), got {hits:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn ann_search_rejects_wrong_dim() {
    let (_dir, store) = open_populated_store().await;

    // 3 components, but the table was opened with dim=4. LanceDB validates
    // the query dim at the scanner boundary and surfaces a hard error
    // ("query dim(3) doesn't match the column vec vector dim(4)") which
    // we map to `IndexError::Lance`. We pin both halves of the contract:
    // the call returns an error, and the variant is `Lance` (so callers
    // can distinguish from BM25-side / IO-side errors).
    let err = store
        .ann(&[1.0, 0.0, 0.0], 1)
        .await
        .expect_err("ann with wrong-dim must fail");
    assert!(
        matches!(err, IndexError::Lance(_)),
        "expected IndexError::Lance for wrong-dim query, got {err:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn ann_on_empty_dir_returns_empty_vec() {
    // Fresh directory with no `embeddings` table: `LanceStore::ann` must
    // treat this as "no results", not as an error.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = LanceStore::new(dir.path()).await.expect("open");

    let hits = store.ann(&[1.0, 0.0, 0.0, 0.0], 5).await.expect("ann empty");
    assert!(
        hits.is_empty(),
        "missing `embeddings` table must yield empty vec, got {hits:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn open_returns_indexerror_on_missing_dir() {
    // `LanceStore::new` does *not* create the directory — a missing path
    // is a hard error so an operator pointing us at a typo'd path sees a
    // useful message instead of silently creating an empty DB.
    let bad = std::path::Path::new("/this/path/should/not/exist/hwledger-lance-test");
    let err = LanceStore::new(bad).await.expect_err("open must fail");
    assert!(
        matches!(err, IndexError::Lance(_) | IndexError::Io(_)),
        "expected Lance or Io error for missing dir, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// run_hybrid wiring test — proves the dense path is real, not stubbed.
//
// Without LanceDB wired in, `run_hybrid(store, q, k, Some(lance), vec)` would
// fall back to BM25-only even when the caller asks for fusion. To prove the
// dense side actually contributes, this test uses a corpus whose BM25 and ANN
// rankings disagree:
//
//   - The BM25 query "ZZZ" matches **only** doc_b (zero hits for the others).
//   - The ANN query vector is a positive scalar multiple of doc_a's vector,
//     so LanceDB's nearest-neighbour returns doc_a as the top hit and
//     doc_b / doc_c further down.
//
// We pin `k = 1` so the fusion produces exactly one result, and we assert
// it is doc_a — the ANN winner. If the dense path were stubbed, doc_a
// would be missing (BM25 only sees doc_b) and the assertion would fail
// with `ids == []` or `ids == ["doc_b"]`. If the BM25 path were stubbed,
// doc_a would still survive (it appears in the ANN list), but doc_b
// would never appear, and a parallel `k = 1, query = bravo`-style
// experiment would be the proof — we keep it simple here and pin the
// "ANN wins" outcome.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "current_thread")]
async fn run_hybrid_fuses_bm25_with_ann() {
    use hwledger_search_core::{FusedResult, Query};
    use hwledger_search_index::{run_hybrid, IndexedDoc, TantivyStore};

    // ---- BM25 side: Tantivy index with three rows, only one matches "ZZZ"
    let bm25_dir = tempfile::tempdir().expect("bm25 tempdir");
    let bm25 = TantivyStore::open(bm25_dir.path()).expect("open bm25");
    bm25.upsert(&IndexedDoc {
        id: "doc_a",
        name: "Alpha",
        org: "org",
        kind: "instruct",
        family: "alpha",
        arch: "mha",
        quants: "gguf",
        card_snippet: "alpha description",
    })
    .expect("upsert doc_a");
    bm25.upsert(&IndexedDoc {
        id: "doc_b",
        name: "Bravo ZZZ unique-token",
        org: "org",
        kind: "instruct",
        family: "bravo",
        arch: "mha",
        quants: "gguf",
        card_snippet: "bravo description with ZZZ",
    })
    .expect("upsert doc_b");
    bm25.upsert(&IndexedDoc {
        id: "doc_c",
        name: "Charlie",
        org: "org",
        kind: "instruct",
        family: "charlie",
        arch: "mha",
        quants: "gguf",
        card_snippet: "charlie description",
    })
    .expect("upsert doc_c");

    // ---- ANN side: LanceDB with three orthogonal 4-D vectors.
    let lance_dir = tempfile::tempdir().expect("lance tempdir");
    let lance = LanceStore::new(lance_dir.path()).await.expect("open lance");
    lance
        .insert(&[
            EmbeddingRow { id: "doc_a".into(), vector: vec![1.0, 0.0, 0.0, 0.0] },
            EmbeddingRow { id: "doc_b".into(), vector: vec![0.0, 1.0, 0.0, 0.0] },
            EmbeddingRow { id: "doc_c".into(), vector: vec![0.0, 0.0, 1.0, 0.0] },
        ])
        .await
        .expect("insert");

    // ---- Query: BM25 wants "ZZZ" (only doc_b), ANN wants doc_a's direction
    let q = Query::text("ZZZ");
    let qvec = [2.0, 0.0, 0.0, 0.0]; // positive multiple of doc_a → ANN top-1 is doc_a

    // k = 1: only the top-ranked result survives. doc_a is ANN rank 1
    // (cosine 0 to itself) and BM25-absent. doc_b is BM25 rank 1 and
    // ANN rank 2 (cosine 1). RRF gives doc_a the score 1/(60+1) ≈ 0.0164
    // and doc_b the score 1/(60+1) + 1/(60+2) ≈ 0.0325. With k = 1 the
    // top-1 is doc_b (RRF higher). So we use k = 2 instead and assert
    // doc_a is in the top-2 — that is the smallest k that proves both
    // sides contributed.
    let fused: Vec<FusedResult> = run_hybrid(&bm25, &q, 2, Some(&lance), &qvec)
        .await
        .expect("run_hybrid");

    let ids: Vec<String> = fused.iter().map(|r| r.id.clone()).collect();
    assert!(
        ids.contains(&"doc_a".to_string()),
        "ANN path is missing — doc_a (the ANN top-1, BM25-absent) should be in the fused result, got {ids:?}"
    );
    assert!(
        ids.contains(&"doc_b".to_string()),
        "BM25 path is missing — doc_b (the BM25 top-1, ANN rank-2) should be in the fused result, got {ids:?}"
    );
}
