//! Integration tests for the Lance ANN (approximate nearest neighbor) reader.
//!
//! When the `lancedb` feature is **off**, the public [`LanceAnn`] type is a
//! zero-cost stub whose [`LanceAnn::ann`] method always returns an empty hit
//! list. When the feature is **on**, [`LanceAnn::ann`] performs a real
//! `nearest_to` query against a LanceDB table written via [`LanceAnn::insert`].
//!
//! These tests pin both halves of that contract so the default build never
//! silently regresses into a non-functional stub, and the feature-gated build
//! never silently regresses into a no-op reader.

use hwledger_search_index::LanceAnn;

/// Run a future on the appropriate runtime.
///
/// The feature-on backend (`lance-datafusion`) requires a Tokio 1.x
/// runtime, so when the feature is enabled we drive futures via
/// `tokio::runtime::Runtime`. Without the feature the implementation is
/// pure Rust and `futures::executor::block_on` is sufficient and faster.
#[inline]
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    #[cfg(feature = "lancedb")]
    {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime")
            .block_on(fut)
    }
    #[cfg(not(feature = "lancedb"))]
    {
        futures::executor::block_on(fut)
    }
}

/// Always-on regression guard: even without the feature, the stub must be
/// importable and `ann` must return no hits (never panic, never produce
/// spurious data).
#[test]
fn lance_ann_search_is_empty_when_feature_off() {
    // Use a writable tempdir so the feature-on `create_dir_all` doesn't
    // fail when run under the lancedb feature. The stub itself ignores
    // the path entirely.
    let dir = tempfile::tempdir().expect("tempdir");
    let ann = block_on(LanceAnn::new(dir.path())).expect("construct");
    let hits = block_on(ann.ann(&[1.0_f32, 2.0, 3.0, 4.0], 5)).expect("ann");
    assert!(hits.is_empty(), "stub should produce no hits; got {hits:?}");
}

/// Always-on regression guard: `k == 0` must yield no hits regardless of the
/// underlying backend.
#[test]
fn lance_ann_k_zero_returns_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ann = block_on(LanceAnn::new(dir.path())).expect("construct");
    let hits = block_on(ann.ann(&[0.0_f32; 4], 0)).expect("ann");
    assert!(hits.is_empty(), "k=0 must produce no hits; got {hits:?}");
}

/// Feature-gated happy path: populate an in-`tempdir` LanceDB table (the
/// task's "in-memory" stand-in) with three well-separated 4-D vectors and
/// verify that the right `id` is returned for each query.
#[cfg(feature = "lancedb")]
#[test]
fn lance_ann_returns_correct_nearest_id() {
    use hwledger_search_index::AnnRow;

    // Tempdir stands in for an "in-memory" LanceDB table: the table
    // lifetime is bound to the directory, and the directory is dropped at
    // end of scope.
    let dir = tempfile::tempdir().expect("tempdir");
    let ann = block_on(LanceAnn::new(dir.path())).expect("open lance store");

    // Three well-separated 4-D vectors, one row each.
    let rows = [
        AnnRow {
            id: "alpha".to_string(),
            vector: vec![1.0_f32, 0.0, 0.0, 0.0],
        },
        AnnRow {
            id: "beta".to_string(),
            vector: vec![0.0_f32, 1.0, 0.0, 0.0],
        },
        AnnRow {
            id: "gamma".to_string(),
            vector: vec![0.0_f32, 0.0, 1.0, 0.0],
        },
    ];
    block_on(ann.insert(&rows)).expect("insert rows");

    // Exact-match queries — nearest id is unambiguous.
    let hits_alpha = block_on(ann.ann(&[1.0_f32, 0.0, 0.0, 0.0], 1)).expect("ann alpha");
    assert_eq!(hits_alpha, vec!["alpha".to_string()]);

    let hits_beta = block_on(ann.ann(&[0.0_f32, 1.0, 0.0, 0.0], 1)).expect("ann beta");
    assert_eq!(hits_beta, vec!["beta".to_string()]);

    let hits_gamma = block_on(ann.ann(&[0.0_f32, 0.0, 1.0, 0.0], 1)).expect("ann gamma");
    assert_eq!(hits_gamma, vec!["gamma".to_string()]);

    // k > 1 returns all three ids ordered by ascending distance.
    let hits_all = block_on(ann.ann(&[1.0_f32, 0.0, 0.0, 0.0], 3)).expect("ann all");
    assert_eq!(hits_all.len(), 3);
    assert_eq!(hits_all[0], "alpha");
}