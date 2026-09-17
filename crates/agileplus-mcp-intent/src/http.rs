// SPDX-License-Identifier: MIT OR Apache-2.0
//! HTTP API server for the intent converter.
//!
//! Endpoints:
//!   POST /convert      — convert a prompt to an intent graph
//!   POST /convert-and-store — convert and optionally store in DB
//!   GET  /health       — health check
//!   GET  /schema       — return the embedded ontology schema

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::converter::convert;
use crate::storage::{open_storage, store_and_summarize, StorageSummary};
use crate::types::{ConvertRequest, ConvertResponse, ErrorResponse};
use crate::validator::validate_and_wrap;

/// Shared application state.
#[derive(Clone)]
pub struct HttpState {
    pub storage: Option<Arc<agileplus_sqlite::SqliteStorageAdapter>>,
}

/// Start the HTTP server.
pub async fn start_http(addr: SocketAddr) -> anyhow::Result<()> {
    let state = HttpState { storage: None };
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "HTTP API listening");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Build the Axum router.
pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/convert", post(convert_handler))
        .route("/convert-and-store", post(convert_and_store_handler))
        .route("/health", get(health_handler))
        .route("/schema", get(schema_handler))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn convert_handler(
    State(_state): State<HttpState>,
    Json(req): Json<ConvertRequest>,
) -> Result<Json<ConvertResponse>, ApiError> {
    let response = convert(&req.prompt, &req.options).map_err(ApiError::from)?;
    let validated = validate_and_wrap(response).map_err(ApiError::from)?;
    Ok(Json(validated))
}

async fn convert_and_store_handler(
    State(state): State<HttpState>,
    Json(req): Json<ConvertRequest>,
) -> Result<Json<ConvertAndStoreResponse>, ApiError> {
    let response = convert(&req.prompt, &req.options).map_err(ApiError::from)?;
    let validated = validate_and_wrap(response).map_err(ApiError::from)?;

    let storage_summary = if req.options.store {
        let db = state
            .storage
            .clone()
            .or_else(|| open_storage().ok().map(Arc::new));
        if let Some(db) = db {
            store_and_summarize(&db, &validated.graph).await.ok()
        } else {
            None
        }
    } else {
        None
    };

    Ok(Json(ConvertAndStoreResponse {
        graph: validated.graph,
        summary: validated.summary,
        storage: storage_summary,
    }))
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "agileplus-mcp-intent",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn schema_handler() -> Json<serde_json::Value> {
    let schema: serde_json::Value = serde_json::from_str(crate::validator::EMBEDDED_SCHEMA)
        .unwrap_or_else(|_| serde_json::json!({"error": "failed to load schema"}));
    Json(schema)
}

/// Combined response when storing is enabled.
#[derive(Debug, serde::Serialize)]
pub struct ConvertAndStoreResponse {
    pub graph: crate::types::IntentGraph,
    pub summary: crate::types::ConversionSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageSummary>,
}

/// Unified API error type.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("conversion failed: {0}")]
    Conversion(#[from] anyhow::Error),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match &self {
            ApiError::Conversion(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!(ErrorResponse {
                    error: e.to_string(),
                    code: "CONVERSION_ERROR".to_string(),
                    details: None,
                }),
            ),
            ApiError::Validation(err) => (StatusCode::UNPROCESSABLE_ENTITY, serde_json::json!(err)),
            ApiError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                serde_json::json!(ErrorResponse {
                    error: msg.clone(),
                    code: "BAD_REQUEST".to_string(),
                    details: None,
                }),
            ),
        };
        (status, Json(body)).into_response()
    }
}

impl From<ErrorResponse> for ApiError {
    fn from(err: ErrorResponse) -> Self {
        ApiError::Validation(err.error)
    }
}
