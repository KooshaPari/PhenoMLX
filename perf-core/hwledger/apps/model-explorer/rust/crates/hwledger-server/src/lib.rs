//! `hwledger-server` — standalone Axum HTTP server bundling every
//! `hwledger-search-*` crate behind a single process.
//!
//! The crate exposes both a binary (`hwledger-server`) and a small library
//! surface (`hwledger_server::router`, `hwledger_server::AppState`,
//! `hwledger_server::run_hybrid`) so that integration tests can mount the
//! same router against an in-process tantivy fixture without spawning a
//! child process.
//!
//! ## Crate map
//!
//! | module                        | role                                                     |
//! |-------------------------------|----------------------------------------------------------|
//! | [`state`]                     | `AppState` shared by every handler.                      |
//! | [`service`]                   | Thin async wrappers around `hwledger-search-index`.      |
//! | [`routes`]                    | Axum router + per-endpoint handler modules.              |
//!
//! ## Configuration
//!
//! The binary reads two environment variables:
//!
//! - `DATA_DIR` — the directory the underlying tantivy store will be opened
//!   from. Required: the process refuses to start without it. If the
//!   directory does not exist yet, the store is created lazily on first
//!   read.
//! - `PORT` — port to listen on. Defaults to `8080`.
//! - `ADMIN_TOKEN` — bearer token required by the admin endpoints (see
//!   [`routes::admin`]). Unset in dev; set to a long random string in prod.

#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

pub mod observability;
pub mod routes;
pub mod service;
pub mod state;

pub use state::AppState;

use axum::Router;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Construct the Axum [`Router`] wired to every route module.
///
/// Exposed publicly so integration tests in `tests/` can mount the same
/// router against an in-memory tantivy store via `tower::ServiceExt::oneshot`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(routes::health::router())
        .merge(routes::search::router())
        .merge(routes::detail::router())
        .merge(routes::ask::router())
        .merge(routes::quants::router())
        .merge(routes::similar::router())
        .merge(routes::for_use_case::router())
        .merge(routes::admin::router())
        .layer(axum::middleware::from_fn(observability::attach_path_to_span))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(Arc::new(state))
}