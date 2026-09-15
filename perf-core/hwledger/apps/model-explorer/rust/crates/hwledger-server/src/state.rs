//! Shared application state passed to every Axum handler.
//!
//! `AppState` owns the only handle into the underlying tantivy store. The
//! store is wrapped in an `Arc` so handler futures can `clone()` cheaply
//! without taking a `&TantivyStore` through the request lifetime.
//!
//! `data_dir` is exposed so admin endpoints can resolve paths (e.g. for a
//! future "export to parquet" route) without round-tripping through the
//! tantivy handle.

use std::path::PathBuf;
use std::sync::Arc;

use hwledger_search_index::TantivyStore;

/// Process-wide state shared by every route handler.
#[derive(Clone)]
pub struct AppState {
    /// Tantivy BM25 store. Cheap to clone (`Arc` internally).
    pub index: Arc<TantivyStore>,
    /// Root data directory the store was opened from.
    pub data_dir: PathBuf,
}

impl AppState {
    /// Build an `AppState` from an already-opened tantivy handle and its
    /// origin directory.
    #[must_use]
    pub fn new(index: Arc<TantivyStore>, data_dir: PathBuf) -> Self {
        Self { index, data_dir }
    }

    /// Borrow the underlying tantivy store.
    #[must_use]
    pub fn store(&self) -> &TantivyStore {
        &self.index
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("data_dir", &self.data_dir)
            .finish()
    }
}