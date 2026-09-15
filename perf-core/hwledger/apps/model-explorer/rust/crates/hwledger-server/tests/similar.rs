//! Integration tests for `GET /v1/models/:id/similar`.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn similar_returns_other_models_dropping_the_seed() {
    let app = common::seeded_app();
    let req = Request::builder()
        .uri("/v1/models/qwen/Qwen2.5-7B-Instruct/similar?limit=5")
        .body(Body::empty())
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["seed"], "qwen/Qwen2.5-7B-Instruct");
    assert_eq!(json["limit"], 5);

    let results = json["results"].as_array().expect("results array");
    for r in results {
        // The seed id itself must never appear in its own similar list.
        assert_ne!(
            r["id"], "hf::qwen/Qwen2.5-7B-Instruct",
            "seed id must be filtered out"
        );
    }
}

#[tokio::test]
async fn similar_defaults_limit_to_ten_when_omitted() {
    let app = common::seeded_app();
    let req = Request::builder()
        .uri("/v1/models/qwen/Qwen2.5-7B-Instruct/similar")
        .body(Body::empty())
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["limit"], 10);
}