//! Integration tests for `GET /healthz`.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn healthz_returns_ok_with_data_dir() {
    let app = common::seeded_app();
    let req = Request::builder()
        .uri("/healthz")
        .body(Body::empty())
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data_dir"], app.data_dir.display().to_string());
}

#[tokio::test]
async fn healthz_works_when_data_dir_does_not_exist_yet() {
    // Open an empty tantivy store at a fresh path. The health probe
    // shouldn't care about the contents of the store.
    let dir = tempfile::tempdir().expect("tempdir");
    let store = hwledger_search_index::TantivyStore::open(dir.path()).expect("open");
    let state = std::sync::Arc::new(hwledger_server::AppState::new(
        std::sync::Arc::new(store),
        dir.path().to_path_buf(),
    ));
    let router = hwledger_server::router((*state).clone());

    let req = Request::builder()
        .uri("/healthz")
        .body(Body::empty())
        .expect("build request");

    let (resp, body) = common::send(router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["status"], "ok");
}