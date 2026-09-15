//! `hwledger-mcp-http` — JSON-RPC 2.0 + SSE entrypoint over HTTP for the
//! hwledger MCP server.
//!
//! This binary is the HTTP/SSE sibling of the stdio `hwledger-mcp`
//! binary. It exposes the same [`McpServer`] dispatcher but over a
//! long-lived axum listener so browser-based MCP clients, server-to-
//! server callers, and any LLM harness that already speaks HTTP+SSE
//! can reach the same six tools without spawning a child process.
//!
//! ## Endpoints
//!
//! - `POST /mcp` — synchronous JSON-RPC 2.0 request → JSON-RPC 2.0
//!   response (per-request).
//! - `GET /sse`  — Server-Sent Events stream. The server emits a
//!   named `endpoint` event with the absolute URL the client should
//!   POST subsequent requests to.
//!
//! ## Environment variables
//!
//! - `DATA_DIR`   — **required**. Directory containing the tantivy
//!   index (same as the stdio binary). Cold-starts a fresh index if
//!   the directory doesn't yet exist.
//! - `HTTP_PORT`  — port to listen on. Defaults to `9000`.
//! - `RUST_LOG`   — standard `tracing-subscriber` env-filter. Defaults
//!   to `info,hwledger_mcp=info`.
//! - `HWLEDGER_MCP_TLS` — when set to `0` / `false`, force the SSE
//!   endpoint URL scheme to `http` even for external hosts. Any
//!   other value forces `https`.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use hwledger_mcp::{backend::ServiceBackend, transport_http};
use hwledger_search_index::TantivyStore;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("hwledger-mcp-http: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    init_tracing();

    // `DATA_DIR` is required — same as the stdio binary, the HTTP
    // transport owns the same Tantivy store and refuses to start
    // without it.
    let data_dir =
        std::env::var("DATA_DIR").context("DATA_DIR must be set to the tantivy index directory")?;
    let data_dir = PathBuf::from(data_dir);

    // `HTTP_PORT` defaults to 9000 per the task spec. We deliberately
    // parse it ourselves (rather than via `clap`) so the binary stays
    // a single env-var interface like the stdio sibling.
    let port: u16 = std::env::var("HTTP_PORT")
        .ok()
        .map(|s| s.parse::<u16>())
        .transpose()
        .context("HTTP_PORT must be a u16")?
        .unwrap_or(9000);

    tracing::info!(
        data_dir = %data_dir.display(),
        port,
        "starting hwledger-mcp-http"
    );

    // Open (or create) the tantivy store at $DATA_DIR. The
    // constructor creates the directory if it doesn't exist, so a
    // cold-start of a freshly-deployed instance just works.
    let store = TantivyStore::open(&data_dir)
        .with_context(|| format!("failed to open tantivy store at {}", data_dir.display()))?;
    let store = Arc::new(store);

    // The `ServiceBackend` owns its own dedicated tokio runtime so
    // the synchronous tools layer can call into the async service
    // layer without restructuring the transport.
    let backend =
        Arc::new(ServiceBackend::new(store).context("failed to construct service backend")?);

    let state = transport_http::HttpState::new(backend);

    let app = transport_http::router(state);

    // Bind to `0.0.0.0:<port>` so the server is reachable from
    // outside the container / VM. Reverse proxies in front of us
    // (nginx, envoy, …) terminate TLS and forward plain HTTP.
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind to {addr}"))?;
    tracing::info!(addr = %listener.local_addr().unwrap_or(addr), "listening");

    axum::serve(listener, app)
        .await
        .context("axum server failed")?;

    Ok(())
}

/// Initialize the `tracing-subscriber` subscriber.
///
/// Idempotent: `try_init` swallows the "already set" error so the
/// binary is safe to embed in test harnesses that construct the
/// runtime more than once.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,hwledger_mcp=info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
