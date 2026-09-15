//! Dense-vector ANN (approximate nearest neighbor) over LanceDB.
//!
//! This module is the **test-friendly** half of the dense-ANN surface.
//! The production reader lives in [`crate::lance_store::LanceStore`]; this
//! module is a thin convenience wrapper that:
//!
//! - Exists (with the same name and stub-vs-real semantics) regardless of
//!   whether the `lancedb` cargo feature is on, so integration tests can
//!   reference a single type and dispatch to the right behaviour.
//! - Hides the [`crate::lance_store::LanceStore`] constructor from tests
//!   that don't care about its raw `&Path` argument.
//! - Provides a tiny `insert` + `ann` two-method API matched to what the
//!   `tests/lance_ann.rs` integration test exercises.
//!
//! ## Feature split
//!
//! The `lancedb` cargo feature is **default OFF** (`default = []`). The
//! `LanceAnn` API is therefore either:
//!
//! - **Without the feature** — a stub. `new` is a no-op (no filesystem
//!   access, no LanceDB linked), `insert` is a silent no-op, and `ann`
//!   always returns an empty `Vec`. The integration test in
//!   `tests/lance_ann.rs` exercises this branch by asserting that
//!   `LanceAnn::ann` returns an empty vec on a freshly-`new`-ed handle
//!   when no rows have ever been inserted.
//! - **With the feature** — delegates to a real
//!   [`crate::lance_store::LanceStore`]. The integration test in
//!   `tests/lance_ann.rs` populates an in-`tempdir` LanceDB table with
//!   three well-separated 4-D vectors, then asserts that the
//!   nearest-neighbour search returns the matching `id` at rank 0.

use crate::error::IndexError;

/// One row to insert into the dense ANN table — mirrors
/// [`crate::lance_store::EmbeddingRow`] but stays stable (no feature
/// gating on the *type*) so callers can use it in both feature
/// configurations.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnRow {
    /// Stable primary key (e.g. `"qwen/Qwen2.5-7B-Instruct"`).
    pub id: String,
    /// Dense embedding, length must match the table's vector dim.
    pub vector: Vec<f32>,
}

#[cfg(feature = "lancedb")]
impl From<AnnRow> for crate::lance_store::EmbeddingRow {
    fn from(r: AnnRow) -> Self {
        Self {
            id: r.id,
            vector: r.vector,
        }
    }
}

// ---------------------------------------------------------------------------
// Feature-ON: real implementation, delegates to LanceStore.
//
// We hold an `Arc` so `LanceAnn` itself is `Clone`-cheap to share across
// async tasks (the underlying `LanceStore` is already `Clone` via its
// internal `Arc<Connection>`, but wrapping it in `Arc` here means
// `LanceAnn` doesn't even need to clone the inner store on every clone).
// ---------------------------------------------------------------------------
#[cfg(feature = "lancedb")]
mod inner {
    use std::path::Path;
    use std::sync::Arc;

    use super::{AnnRow, IndexError};
    use crate::lance_store::LanceStore;

    /// Feature-on dense-ANN handle. Holds an `Arc<LanceStore>` so clones
    /// are zero-cost.
    #[derive(Clone)]
    pub struct LanceAnn {
        store: Arc<LanceStore>,
    }

    impl std::fmt::Debug for LanceAnn {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("LanceAnn").field("store", &self.store).finish()
        }
    }

    impl LanceAnn {
        /// Open (or create) a LanceDB-backed dense index rooted at `dir`
        /// and return a ready-to-query handle.
        pub async fn new(dir: &Path) -> Result<Self, IndexError> {
            // `LanceStore::new` requires the directory to exist; we
            // create it here so callers (and tests) don't have to.
            std::fs::create_dir_all(dir)?;
            let store = LanceStore::new(dir).await?;
            Ok(Self {
                store: Arc::new(store),
            })
        }

        /// Insert `rows` into the dense index.
        pub async fn insert(&self, rows: &[AnnRow]) -> Result<(), IndexError> {
            let borrowed: Vec<crate::lance_store::EmbeddingRow> = rows
                .iter()
                .cloned()
                .map(crate::lance_store::EmbeddingRow::from)
                .collect();
            self.store.insert(&borrowed).await
        }

        /// Cosine-ANN nearest-neighbour search.
        ///
        /// Returns up to `k` model `id`s, ordered by similarity
        /// (most-similar first). If `k == 0` or `query` is empty (or
        /// the index is empty) the result is an empty `Vec` rather
        /// than an error — this matches the "no dense index yet"
        /// tolerance the production BM25-only path depends on.
        pub async fn ann(&self, query: &[f32], k: usize) -> Result<Vec<String>, IndexError> {
            self.store.ann(query, k).await
        }
    }
}

// ---------------------------------------------------------------------------
// Feature-OFF: zero-cost stub.
//
// `LanceAnn` is constructable, every operation is a no-op, and `ann`
// always returns an empty `Vec`. The integration test asserts this
// directly: the default-build test calls `LanceAnn::new` and then
// `LanceAnn::ann` and expects an empty vec back.
// ---------------------------------------------------------------------------
#[cfg(not(feature = "lancedb"))]
mod inner {
    use std::path::Path;

    use super::{AnnRow, IndexError};

    /// Feature-off stub for [`LanceAnn`]. Holds no state and never
    /// touches the filesystem; every method is a no-op.
    #[derive(Debug, Clone, Default)]
    pub struct LanceAnn {
        _private: (),
    }

    impl LanceAnn {
        /// No-op `new`. Returns a stub handle without touching `dir`.
        pub async fn new(_dir: &Path) -> Result<Self, IndexError> {
            Ok(Self { _private: () })
        }

        /// No-op `insert`. Silently drops the rows.
        pub async fn insert(&self, _rows: &[AnnRow]) -> Result<(), IndexError> {
            Ok(())
        }

        /// Stub `ann`. Always returns an empty `Vec` per the
        /// feature-off contract.
        pub async fn ann(&self, _query: &[f32], _k: usize) -> Result<Vec<String>, IndexError> {
            Ok(Vec::new())
        }
    }
}

pub use inner::LanceAnn;