// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for the AgilePlus HTTP API.
//!
//! These tests spin up a real axum test server backed by in-memory mock
//! implementations of all ports. No external dependencies are required.
//!
//! Run with: `cargo test -p agileplus-api`
//!
//! Traceability: WP15-T090

#![allow(dead_code)]

#[path = "api_integration/support/mod.rs"]
mod support;

use axum::http::StatusCode;
use serde_json;

use support::{setup_test_server, TEST_API_KEY};
// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_no_auth_required() {
    let server = setup_test_server().await;
    // /health returns simple health; /detailed-health returns full status
    let resp = server.get("/detailed-health").await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    // Detailed health endpoint returns "healthy" or "degraded".
    let status = body["status"].as_str().expect("status field present");
    assert!(
        status == "healthy" || status == "degraded",
        "unexpected health status: {status}"
    );
    // Timestamp and services must be present.
    assert!(body["timestamp"].is_string());
    assert!(body["services"].is_object());
}

#[tokio::test]
async fn info_no_auth_required() {
    let server = setup_test_server().await;
    let resp = server.get("/info").await;
    resp.assert_status_ok();
}

#[tokio::test]
async fn list_features_requires_auth() {
    let server = setup_test_server().await;
    let resp = server.get("/api/v1/features").await;
    resp.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_features_with_valid_key() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/features")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    let arr = body.as_array().unwrap();
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["slug"], "test-feature");
}

#[tokio::test]
async fn list_features_invalid_key_returns_401() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/features")
        .add_header("X-API-Key", "wrong-key")
        .await;
    resp.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_feature_found() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/features/test-feature")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["slug"], "test-feature");
    assert_eq!(body["name"], "Test Feature");
}

#[tokio::test]
async fn get_feature_not_found() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/features/nonexistent")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_work_package_found() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/work-packages/1")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["id"], 1);
    assert_eq!(body["title"], "WP01");
}

#[tokio::test]
async fn get_work_package_not_found() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/work-packages/999")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_audit_trail() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/features/test-feature/audit")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["actor"], "system");
}

#[tokio::test]
async fn verify_audit_chain_valid() {
    let server = setup_test_server().await;
    let resp = server
        .post("/api/v1/features/test-feature/audit/verify")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["chain_valid"], true);
    assert_eq!(body["entries_verified"], 2);
}

#[tokio::test]
async fn get_governance() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/features/test-feature/governance")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["version"], 1);
    assert_eq!(body["feature_id"], 1);
}

#[tokio::test]
async fn trigger_validate() {
    let server = setup_test_server().await;
    let resp = server
        .post("/api/v1/features/test-feature/validate")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["feature_slug"], "test-feature");
    assert_eq!(body["compliant"], true); // no rules → all satisfied
}

#[tokio::test]
async fn response_content_type_is_json() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/features")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        ct.contains("application/json"),
        "Expected application/json, got: {ct}"
    );
}

// ── Domain-wiring tests: Projects ─────────────────────────────────────────────

#[tokio::test]
async fn list_projects_happy_path() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/projects")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    let arr = body.as_array().unwrap();
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["slug"], "test-project");
    assert_eq!(arr[0]["name"], "Test Project");
}

#[tokio::test]
async fn get_project_found() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/projects/test-project")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["slug"], "test-project");
}

#[tokio::test]
async fn get_project_not_found_returns_404() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/projects/nonexistent-project")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_project_happy_path() {
    let server = setup_test_server().await;
    let resp = server
        .post("/api/v1/projects")
        .add_header("X-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "name": "New Project", "slug": "new-project" }))
        .await;
    resp.assert_status(StatusCode::CREATED);
    let body: serde_json::Value = resp.json();
    assert_eq!(body["slug"], "new-project");
    assert_eq!(body["name"], "New Project");
}

#[tokio::test]
async fn create_project_invalid_slug_returns_400() {
    let server = setup_test_server().await;
    let resp = server
        .post("/api/v1/projects")
        .add_header("X-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "name": "Bad Project", "slug": "Invalid SLUG!" }))
        .await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_epics_for_project() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/projects/test-project/epics")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    let arr = body.as_array().unwrap();
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["title"], "Test Epic");
    assert_eq!(arr[0]["project_id"], 1);
}

// ── Domain-wiring tests: Epics ────────────────────────────────────────────────

#[tokio::test]
async fn get_epic_happy_path() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/epics/1")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["id"], 1);
    assert_eq!(body["title"], "Test Epic");
    assert_eq!(body["status"], "active");
}

#[tokio::test]
async fn get_epic_not_found_returns_404() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/epics/999")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_epic_happy_path() {
    let server = setup_test_server().await;
    let resp = server
        .post("/api/v1/epics")
        .add_header("X-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "project_id": 1, "title": "New Epic" }))
        .await;
    resp.assert_status(StatusCode::CREATED);
    let body: serde_json::Value = resp.json();
    assert_eq!(body["title"], "New Epic");
    assert_eq!(body["status"], "backlog");
}

#[tokio::test]
async fn create_epic_empty_title_returns_400() {
    let server = setup_test_server().await;
    let resp = server
        .post("/api/v1/epics")
        .add_header("X-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "project_id": 1, "title": "   " }))
        .await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn transition_epic_invalid_transition_returns_409() {
    let server = setup_test_server().await;
    // Epic 1 is Active; Active -> Done is not a valid transition
    let resp = server
        .post("/api/v1/epics/1/transition")
        .add_header("X-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "target_status": "done" }))
        .await;
    resp.assert_status(StatusCode::CONFLICT);
}

#[tokio::test]
async fn list_stories_for_epic() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/epics/1/stories")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    let arr = body.as_array().unwrap();
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["title"], "Test Story");
}

// ── Domain-wiring tests: Stories ─────────────────────────────────────────────

#[tokio::test]
async fn get_story_happy_path() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/stories/1")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["id"], 1);
    assert_eq!(body["title"], "Test Story");
    assert_eq!(body["status"], "todo");
}

#[tokio::test]
async fn get_story_not_found_returns_404() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/stories/999")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_story_happy_path() {
    let server = setup_test_server().await;
    let resp = server
        .post("/api/v1/stories")
        .add_header("X-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "epic_id": 1, "project_id": 1, "title": "New Story", "points": 5 }))
        .await;
    resp.assert_status(StatusCode::CREATED);
    let body: serde_json::Value = resp.json();
    assert_eq!(body["title"], "New Story");
    assert_eq!(body["points"], 5);
    assert_eq!(body["status"], "todo");
}

#[tokio::test]
async fn create_story_zero_points_returns_400() {
    let server = setup_test_server().await;
    let resp = server
        .post("/api/v1/stories")
        .add_header("X-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "epic_id": 1, "project_id": 1, "title": "Bad Story", "points": 0 }))
        .await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn transition_story_invalid_transition_returns_409() {
    let server = setup_test_server().await;
    // Story 1 is Todo; Todo -> Done is not a valid transition
    let resp = server
        .post("/api/v1/stories/1/transition")
        .add_header("X-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "target_status": "done" }))
        .await;
    resp.assert_status(StatusCode::CONFLICT);
}

// ── Domain-wiring tests: Users ────────────────────────────────────────────────

#[tokio::test]
async fn list_users_happy_path() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/users")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    let arr = body.as_array().unwrap();
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["display_name"], "Alice");
    assert_eq!(arr[0]["email"], "alice@example.com");
}

#[tokio::test]
async fn get_user_happy_path() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/users/1")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["id"], 1);
    assert_eq!(body["email"], "alice@example.com");
}

#[tokio::test]
async fn get_user_not_found_returns_404() {
    let server = setup_test_server().await;
    let resp = server
        .get("/api/v1/users/999")
        .add_header("X-API-Key", TEST_API_KEY)
        .await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_user_happy_path() {
    let server = setup_test_server().await;
    let resp = server
        .post("/api/v1/users")
        .add_header("X-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "display_name": "Bob", "email": "bob@example.com", "role": "member" }))
        .await;
    resp.assert_status(StatusCode::CREATED);
    let body: serde_json::Value = resp.json();
    assert_eq!(body["display_name"], "Bob");
    assert_eq!(body["role"], "member");
}

#[tokio::test]
async fn create_user_invalid_email_returns_400() {
    let server = setup_test_server().await;
    let resp = server
        .post("/api/v1/users")
        .add_header("X-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "display_name": "Charlie", "email": "not-an-email" }))
        .await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_user_invalid_role_returns_400() {
    let server = setup_test_server().await;
    let resp = server
        .post("/api/v1/users")
        .add_header("X-API-Key", TEST_API_KEY)
        .json(&serde_json::json!({ "display_name": "Dave", "email": "dave@example.com", "role": "superadmin" }))
        .await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}

// ── FR-AGP-015 — OpenTelemetry request-span middleware ────────────────────

/// AC (FR-AGP-015): OTel request-span middleware wraps a handler.
///
/// The `OtelTracingLayer` must not break the response pipeline.  We verify:
/// - The handler still returns its normal response (200 OK with a JSON body).
/// - No panic occurs during span creation / recording.
///
/// No live OTLP collector is required; the middleware uses `tracing` spans
/// which are no-op when no subscriber exports them.
#[tokio::test]
async fn otel_request_span_middleware_wraps_handler() {
    use agileplus_api::middleware::otel::opentelemetry_tracing_layer;
    use axum::{Json, Router, routing::get};
    use axum_test::TestServer;

    // Minimal router with the OTel layer applied — mirrors production wiring.
    let app = Router::new()
        .route(
            "/ping",
            get(|| async { Json(serde_json::json!({"ok": true})) }),
        )
        .layer(opentelemetry_tracing_layer());

    let server = TestServer::new(app);
    let resp = server.get("/ping").await;
    resp.assert_status_ok();

    // Span attributes (method + path) are recorded on the span; verify the
    // body is still the expected JSON (pipeline was not disrupted).
    let body: serde_json::Value = resp.json();
    assert_eq!(body["ok"], serde_json::json!(true));
}

/// AC (FR-AGP-015): OTel layer propagates W3C traceparent header without panic.
#[tokio::test]
async fn otel_request_span_propagates_traceparent() {
    use agileplus_api::middleware::otel::opentelemetry_tracing_layer;
    use axum::{Json, Router, routing::get};
    use axum_test::TestServer;

    let app = Router::new()
        .route(
            "/ping",
            get(|| async { Json(serde_json::json!({"ok": true})) }),
        )
        .layer(opentelemetry_tracing_layer());

    let server = TestServer::new(app);
    let resp = server
        .get("/ping")
        .add_header(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .await;
    resp.assert_status_ok();
}
