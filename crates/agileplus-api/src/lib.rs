//! agileplus-api — HTTP API server for AgilePlus project management
//!
//! Exports routes, handlers, state management, and middleware for the axum-based HTTP server.

pub mod api_key;
pub mod error;
pub mod middleware;
pub mod openapi;
pub mod responses;
pub mod router;
pub mod routes;
pub mod state;

pub use router::{create_router, start_api};
pub use state::AppState;
