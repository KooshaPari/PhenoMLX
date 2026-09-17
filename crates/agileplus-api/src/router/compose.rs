//! Router composition and HTTP server startup.
//!
//! Route layout:
//!
//! Public (no auth):
//!   GET  /health    — simple health check
//!   GET  /detailed-health — detailed health check (T070)
//!   GET  /info      — API metadata
//!
//! Protected (Bearer token or X-API-Key):
//!   GET  /api/v1/features                           — list features (T066)
//!   POST /api/v1/features                           — create feature (T066)
//!   GET  /api/v1/features/:slug                     — get feature (T066)
//!   PATCH /api/v1/features/:slug                    — update feature (T066)
//!   POST /api/v1/features/:slug/transition          — transition feature state (T066)
//!   GET  /api/v1/features/:slug/work-packages       — list WPs (T067)
//!   POST /api/v1/features/:slug/work-packages       — create WP (T067)
//!   GET  /api/v1/work-packages/:id                  — get WP (T067)
//!   PATCH /api/v1/work-packages/:id                 — update WP (T067)
//!   POST /api/v1/work-packages/:id/transition       — transition WP state (T067)
//!   GET  /api/v1/features/:slug/audit               — audit trail
//!   POST /api/v1/features/:slug/audit/verify        — verify audit chain
//!   GET  /api/v1/features/:slug/governance          — governance contract
//!   POST /api/v1/features/:slug/validate            — run governance validation
//!   GET  /api/v1/events                             — query events (T068)
//!   GET  /api/v1/events/:id                         — single event (T068)
//!   GET  /api/v1/stream                             — SSE real-time events (T069)
//!
//! Traceability: WP11-T065..T070

use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::get;
use axum::{Router, middleware};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use agileplus_domain::ports::vcs::VcsPort;
use agileplus_domain::ports::{ContentStoragePort, ObservabilityPort, StoragePort};

use crate::routes::{
    audit, branch, cycle, epics, events, features, governance, module, projects, stories, stream,
    users, work_packages, worktree,
};
use crate::state::AppState;

use super::handlers::info_handler;
use super::health::{health_handler, simple_health_handler};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Build the axum [`Router`] with all routes, middleware, and shared state.
pub fn create_router<S, V, O>(state: AppState<S, V, O>) -> Router
where
    S: StoragePort + ContentStoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let credentials = Arc::clone(&state.credentials);

    // Public routes -- no auth middleware.
    let public = Router::new()
        .route("/health", get(simple_health_handler))
        .route("/detailed-health", get(health_handler::<S, V, O>))
        .route("/info", get(info_handler))
        // HTML dashboard pages (no auth for browser access)
        .route("/modules", get(module::module_tree_page::<S, V, O>))
        .route("/cycles", get(cycle::cycle_kanban_page::<S, V, O>))
        .route("/cycles/{id}", get(cycle::cycle_detail_page::<S, V, O>))
        .with_state(state.clone());

    // Protected routes — all require a valid API key.
    let protected = Router::new()
        // Feature CRUD + transitions
        .nest("/api/v1/features", features::routes::<S, V, O>())
        // Work-package CRUD + transitions
        .nest("/api/v1/work-packages", work_packages::routes::<S, V, O>())
        // Work-package routes nested under features
        .nest(
            "/api/v1/features",
            work_packages::feature_wp_routes::<S, V, O>(),
        )
        // Governance and audit nested under features
        .nest("/api/v1/features", governance::routes::<S, V, O>())
        .nest("/api/v1/features", audit::routes::<S, V, O>())
        // Module and Cycle API routes
        .nest("/api/modules", module::routes::<S, V, O>())
        .nest("/api/cycles", cycle::routes::<S, V, O>())
        .nest("/api/v1/branches", branch::routes::<S, V, O>())
        .nest("/api/v1/worktrees", worktree::routes::<S, V, O>())
        // Event query endpoints
        .nest("/api/v1/events", events::routes::<S, V, O>())
        // SSE streaming
        .route("/api/v1/stream", get(stream::stream_events::<S, V, O>))
        // Domain: projects, epics, stories, users
        .nest("/api/v1/projects", projects::routes::<S, V, O>())
        .nest("/api/v1/epics", epics::routes::<S, V, O>())
        .nest("/api/v1/stories", stories::routes::<S, V, O>())
        .nest("/api/v1/users", users::routes::<S, V, O>())
        .layer(middleware::from_fn_with_state(
            credentials,
            crate::middleware::auth::validate_api_key,
        ))
        .with_state(state);

    Router::new()
        .merge(public)
        .merge(protected)
        // NOTE: "templates/static" is relative to the process CWD, which must
        // be the workspace root (where the `templates/` directory lives).
        // A future improvement could use a compile-time or env-based path.
        .nest_service("/static", ServeDir::new("templates/static"))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}

/// Start the HTTP API server, binding to `addr`.
pub async fn start_api<S, V, O>(addr: SocketAddr, state: AppState<S, V, O>) -> Result<(), BoxError>
where
    S: StoragePort + ContentStoragePort + Send + Sync + 'static,
    V: VcsPort + Send + Sync + 'static,
    O: ObservabilityPort + Send + Sync + 'static,
{
    let app = create_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "HTTP API listening");
    axum::serve(listener, app).await?;
    Ok(())
}
