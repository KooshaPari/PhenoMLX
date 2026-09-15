//! Thin wrappers over `TantivyStore` that the CLI needs but the
//! search-index crate does not own.
//!
//! - [`SharedStore`] — an `Arc<TantivyStore>` clone-able handle that the
//!   `run_hybrid_blocking` helper can move into a tokio future.
//! - [`TantivySeedSink`] — implements `SeedSink` by forwarding each upsert
//!   into the underlying tantivy store. The sink is `&mut self`-shaped so
//!   it composes with the existing seed-builder signature.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use hwledger_search_core::RawModel;
use hwledger_search_index::{IndexedModel, TantivyStore};
use hwledger_search_ingest::SeedSink;

/// Cheap-to-clone handle around a [`TantivyStore`].
#[derive(Clone)]
pub struct SharedStore {
    inner: Arc<TantivyStore>,
}

impl SharedStore {
    /// Wrap an owned store in the shared handle.
    pub fn new(store: TantivyStore) -> Self {
        Self { inner: Arc::new(store) }
    }

    /// Borrow the underlying tantivy store.
    pub fn as_ref(&self) -> &TantivyStore {
        &self.inner
    }

    /// Clone the inner `Arc`.
    pub fn arc(&self) -> Arc<TantivyStore> {
        Arc::clone(&self.inner)
    }
}

impl std::ops::Deref for SharedStore {
    type Target = TantivyStore;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Open (or create) a tantivy store at `path` and wrap it in a [`SharedStore`].
pub fn open_or_create_store(path: &Path) -> Result<SharedStore> {
    let store = TantivyStore::open(path)
        .with_context(|| format!("failed to open tantivy store at {}", path.display()))?;
    Ok(SharedStore::new(store))
}

/// `SeedSink` adapter that forwards `upsert` into a [`TantivyStore`].
pub struct TantivySeedSink {
    /// Underlying tantivy handle (cloned from the parent `SharedStore`).
    store: Arc<TantivyStore>,
}

impl std::fmt::Debug for TantivySeedSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TantivySeedSink").finish()
    }
}

impl TantivySeedSink {
    /// Wrap the store in a `SeedSink` shim.
    pub fn new(store: Arc<TantivyStore>) -> Self {
        Self { store }
    }
}

impl SeedSink for TantivySeedSink {
    fn upsert(&mut self, raw: &RawModel) -> Result<(), String> {
        let id = raw.key();
        let name = raw.id.clone();
        let org = raw
            .config_json
            .as_ref()
            .and_then(|c| c.get("model_type"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let kind = infer_kind(&raw.tree_entries);
        let family = raw
            .config_json
            .as_ref()
            .and_then(|c| c.get("model_type"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let arch = raw
            .config_json
            .as_ref()
            .and_then(|c| c.get("architectures"))
            .and_then(serde_json::Value::as_array)
            .and_then(|a| a.first())
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let quants = infer_quants(&raw.tree_entries);
        let card_snippet: String = raw
            .card_text
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(2000)
            .collect();
        let model = IndexedModel {
            id,
            name,
            org,
            kind,
            family,
            arch,
            quants,
            card_snippet,
        }
        .truncated();

        hwledger_search_index::upsert_model(&self.store, &model)
            .map_err(|e| e.to_string())
    }
}

/// Build the appropriate sink for a [`SharedStore`].
pub fn seed_sink_for(store: &SharedStore) -> TantivySeedSink {
    TantivySeedSink::new(store.arc())
}

/// Commit + log a one-line summary for a completed seed run.
pub fn write_store(_store: &TantivyStore, _report: &hwledger_search_ingest::SeedReport) {
    // The seed sink already commits at the end of the run; this helper
    // exists so the call site reads cleanly. Future phases can plug in a
    // manifest write here without touching main.rs.
}

/// Heuristic: map a list of tree-entry paths onto a coarse `ModelKind` label.
fn infer_kind(tree: &[String]) -> String {
    let joined = tree.join(" ");
    if joined.contains(".gguf") || joined.contains(".bin") {
        "quant".to_string()
    } else if joined.contains("lora") {
        "adapter".to_string()
    } else if joined.contains("chat_template.json") {
        "chat".to_string()
    } else {
        "base".to_string()
    }
}

/// Heuristic: extract quant format tokens from the tree listing.
fn infer_quants(tree: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for entry in tree {
        let lower = entry.to_ascii_lowercase();
        for tag in ["gguf", "gptq", "awq", "exl2", "safetensors", "bin"] {
            if lower.contains(tag) && seen.insert(tag.to_string()) {
                out.push(tag.to_string());
            }
        }
    }
    out
}

/// Resolve the configured index directory, defaulting to `./hwledger-index`.
#[allow(dead_code)]
pub fn default_index_dir() -> PathBuf {
    PathBuf::from("./hwledger-index")
}