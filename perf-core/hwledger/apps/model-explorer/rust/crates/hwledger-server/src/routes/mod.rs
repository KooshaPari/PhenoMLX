//! Per-endpoint Axum handler modules.
//!
//! Each submodule exposes a `router() -> axum::Router<Arc<AppState>>`
//! that the top-level [`crate::router`] function merges into the final
//! app router. Handlers stay thin — the only logic they own is HTTP-shape
//! parsing and response rendering; everything else lives in
//! [`crate::service`].

pub mod admin;
pub mod ask;
pub mod detail;
pub mod for_use_case;
pub mod health;
pub mod quants;
pub mod search;
pub mod similar;