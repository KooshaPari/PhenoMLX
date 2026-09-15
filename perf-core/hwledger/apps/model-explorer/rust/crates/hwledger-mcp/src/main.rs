//! `hwledger-mcp` — JSON-RPC 2.0 stdio entrypoint for the hwledger MCP server.
//!
//! Reads one JSON-RPC 2.0 message per line from stdin and writes
//! responses to stdout. Run with no arguments; the binary is intended
//! to be launched by an MCP-aware client (e.g. Claude Desktop, an LLM
//! agent harness) as a child process.
//!
//! ## Environment variables
//!
//! - `DATA_DIR` — **required**. Directory containing (or to receive) the
//!   tantivy index. The store is opened (or created) at startup; the
//!   process exits non-zero on a missing / unreadable directory.
//! - `RUST_LOG` — standard `tracing-subscriber` env-filter. Defaults to
//!   `info,hwledger_mcp=info`.

use std::io::{self, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use hwledger_mcp::{backend::ServiceBackend, transport, McpServer, McpState};
use hwledger_search_index::TantivyStore;
use tracing_subscriber::EnvFilter;

fn main() {
    if let Err(e) = run() {
        // Stdio is the only I/O we have; surface the failure there as a
        // last-resort JSON-RPC error message so the client can observe
        // what went wrong.
        eprintln!("hwledger-mcp: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    init_tracing();

    let data_dir =
        std::env::var("DATA_DIR").context("DATA_DIR must be set to the tantivy index directory")?;
    let data_dir = PathBuf::from(data_dir);

    tracing::info!(data_dir = %data_dir.display(), "starting hwledger-mcp");

    // Open (or create) the tantivy store at $DATA_DIR. The constructor
    // creates the directory if it doesn't exist, so a cold-start of a
    // freshly-deployed instance just works.
    let store = TantivyStore::open(&data_dir)
        .with_context(|| format!("failed to open tantivy store at {}", data_dir.display()))?;
    let store = Arc::new(store);

    // The `ServiceBackend` owns its own dedicated tokio runtime so the
    // synchronous stdio loop can call into the async service layer
    // without restructuring the transport.
    let backend =
        Arc::new(ServiceBackend::new(store).context("failed to construct service backend")?);

    let server = McpServer::new();
    let mut state = McpState::new(backend);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let reader = BufReader::new(stdin.lock());

    if let Err(e) = transport::run_stdio(&server, &mut state, reader, &mut stdout) {
        return Err(anyhow::Error::new(e).context("stdio transport failed"));
    }
    Ok(())
}

/// Initialize the `tracing-subscriber` subscriber.
///
/// Idempotent: `try_init` swallows the "already set" error so the binary
/// is safe to embed in test harnesses that construct the runtime more
/// than once.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,hwledger_mcp=info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}
