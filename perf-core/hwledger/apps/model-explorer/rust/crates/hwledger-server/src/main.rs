//! `hwledger-server` — standalone Axum binary bundling every
//! `hwledger-search-*` crate.
//!
//! ## Environment variables
//!
//! - `DATA_DIR`  — **required**. Directory containing (or to receive) the
//!   tantivy index. The store is opened (or created) at startup.
//! - `PORT`      — port to listen on. Defaults to `8080`.
//! - `ADMIN_TOKEN` — bearer token required by `POST /v1/admin/*`. Unset
//!   in dev; set to a long random string in prod. When unset, every
//!   admin request is rejected with `401` so a misconfigured deployment
//!   never silently grants access.
//! - `RUST_LOG`  — standard `tracing-subscriber` env-filter. Defaults to
//!   `info`.
//!
//! ## Storage backend
//!
//! We open `TantivyStore` from the `hwledger-search-index` crate at
//! `$DATA_DIR` directly. There is no `SearchIndex::open(tantivy_dir,
//! lance_dir)` in the current API — the LanceDB dense index lands in a
//! later phase — and no `TantivyStore::in_memory()` either: the
//! `TantivyStore::open` constructor handles a non-existent directory by
//! creating it. We use that path for both cold-start and warm-restart.
//!
//! ## Routes
//!
//! See `routes::` for the per-endpoint handler modules. The aggregate
//! router is built by `hwledger_server::router` and bound to `0.0.0.0:$PORT`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use hwledger_search_index::TantivyStore;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use hwledger_server::{router, service, AppState};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    // Cache the admin token up-front. The handler reads it from a
    // process-local static so we never hit `std::env::var` from a hot
    // request path.
    service::init_admin_token();

    let data_dir = std::env::var("DATA_DIR")
        .context("DATA_DIR must be set to the tantivy index directory")?;
    let data_dir = PathBuf::from(data_dir);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let bind = format!("0.0.0.0:{port}");
    tracing::info!(data_dir = %data_dir.display(), port, "starting hwledger-server");

    // Open (or create) the tantivy store at $DATA_DIR. The constructor
    // creates the directory if it doesn't exist, so a cold-start of a
    // freshly-deployed instance just works.
    let store = TantivyStore::open(&data_dir)
        .with_context(|| format!("failed to open tantivy store at {}", data_dir.display()))?;
    let state = AppState::new(Arc::new(store), data_dir);
    let app = router(state);

    let listener = TcpListener::bind(&bind)
        .await
        .with_context(|| format!("failed to bind {bind}"))?;
    tracing::info!(addr = %bind, "listening");

    // Graceful shutdown on SIGINT / SIGTERM.
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum::serve failed")?;
    Ok(())
}

/// Initialize the `tracing-subscriber` subscriber.
///
/// Idempotent: `try_init` swallows the "already set" error so the binary
/// is safe to embed in test harnesses that construct the runtime more
/// than once.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,hwledger_server=info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

/// Resolve a shutdown signal (`Ctrl-C` on every platform; `SIGTERM` on
/// Unix via `tokio::signal::unix`).
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut s) = signal(SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received SIGINT, shutting down"),
        _ = terminate => tracing::info!("received SIGTERM, shutting down"),
    }
}