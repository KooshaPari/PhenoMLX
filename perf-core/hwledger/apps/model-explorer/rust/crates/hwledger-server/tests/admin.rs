//! Integration tests for admin routes (`/v1/admin/*`).
//!
//! These cover:
//!
//! - request without `ADMIN_TOKEN` set → `401`
//! - request with wrong token → `401`
//! - request with correct token → `200` and the expected JSON shape
//! - collapsing a hit slice returns `CollapsedHit` groups
//!
//! ## Parallelism note
//!
//! The admin-token store is process-local. Because the cargo test runner
//! executes these `#[tokio::test]` functions in parallel threads by
//! default, we serialize them with a single shared mutex. That way the
//! three tests below can each assume it owns the token store for the
//! duration of its own `oneshot` round-trip.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use std::sync::Mutex;

/// Global lock around the admin-token store. Held for the duration of
/// each test in this file so the parallel test runner doesn't observe
/// interleaved `set_admin_token_for_testing` calls.
static ADMIN_LOCK: Mutex<()> = Mutex::new(());

/// RAII guard that restores the token to `None` on drop.
struct TokenGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl TokenGuard {
    fn set(token: Option<&str>) -> Self {
        let lock = ADMIN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        hwledger_server::service::set_admin_token_for_testing(token.map(str::to_string));
        Self { _lock: lock }
    }
}

impl Drop for TokenGuard {
    fn drop(&mut self) {
        hwledger_server::service::set_admin_token_for_testing(None);
    }
}

fn collapse_body() -> serde_json::Value {
    json!({
        "hits": [
            { "id": "a-Q4_K_M", "score": 1.0 },
            { "id": "a-Q5_K_M", "score": 0.9 },
            { "id": "a-Q8_0", "score": 0.8 },
            { "id": "b",        "score": 0.5 },
        ]
    })
}

fn collapse_request(uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    for (k, v) in headers {
        builder = builder.header(*k, *v);
    }
    builder
        .body(Body::from(serde_json::to_vec(&collapse_body()).unwrap()))
        .expect("build request")
}

#[tokio::test]
async fn admin_collapse_rejects_request_without_admin_token_env_var() {
    let _guard = TokenGuard::set(None);

    let app = common::seeded_app();
    let req = collapse_request("/v1/admin/collapse", &[]);
    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "got {}: {}",
        resp.status(),
        String::from_utf8_lossy(&body)
    );
}

#[tokio::test]
async fn admin_collapse_rejects_wrong_token() {
    let _guard = TokenGuard::set(Some("correct-token"));

    let app = common::seeded_app();
    let req = collapse_request("/v1/admin/collapse", &[("x-admin-token", "wrong-token")]);
    let (resp, _) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_collapse_accepts_correct_token_and_groups_variants() {
    let _guard = TokenGuard::set(Some("secret"));

    let app = common::seeded_app();
    let req = collapse_request(
        "/v1/admin/collapse",
        &[("x-admin-token", "secret")],
    );
    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    let groups = json["groups"].as_array().expect("groups array");
    assert_eq!(groups.len(), 2, "expected `a` and `b` collapsed groups");
    let base_ids: Vec<&str> = groups
        .iter()
        .map(|g| g["base_id"].as_str().unwrap())
        .collect();
    assert!(base_ids.contains(&"a"));
    assert!(base_ids.contains(&"b"));
}