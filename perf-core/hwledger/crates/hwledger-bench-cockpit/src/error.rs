use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::fmt;

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppError {
    BadRequest(String),
    NotFound(String),
    ServiceUnavailable(String),
    Internal(String),
    BadGateway(String),
    MethodNotAllowed(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::BadRequest(m) => write!(f, "bad_request: {}", m),
            AppError::NotFound(m) => write!(f, "not_found: {}", m),
            AppError::ServiceUnavailable(m) => write!(f, "service_unavailable: {}", m),
            AppError::Internal(m) => write!(f, "internal: {}", m),
            AppError::BadGateway(m) => write!(f, "bad_gateway: {}", m),
            AppError::MethodNotAllowed(m) => write!(f, "method_not_allowed: {}", m),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            AppError::ServiceUnavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, m),
            AppError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
            AppError::BadGateway(m) => (StatusCode::BAD_GATEWAY, m),
            AppError::MethodNotAllowed(m) => (StatusCode::METHOD_NOT_ALLOWED, m),
        };
        (status, Json(json!({"error": message}))).into_response()
    }
}
