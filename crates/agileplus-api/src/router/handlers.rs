//! Public handler functions for info and metadata endpoints.

use axum::Json;

/// `GET /info` — API metadata (name, version).
pub async fn info_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "agileplus-api",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
