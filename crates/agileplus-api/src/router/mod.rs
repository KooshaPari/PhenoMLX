//! axum router factory and HTTP server startup.
//!
//! Decomposed into modules by concern:
//! - `compose` — router assembly and server startup
//! - `health` — health check handlers and service probes
//! - `handlers` — metadata and utility endpoints

mod compose;
mod handlers;
mod health;

// Re-export public API
pub use compose::{create_router, start_api};
