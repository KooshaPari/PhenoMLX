//! Integration tests for `POST /v1/ask` (RAG v1 stub).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn ask_returns_answer_and_context() {
    let app = common::seeded_app();
    let body = serde_json::json!({
        "question": "What is the latest Qwen model?",
        "limit": 3,
    });
    let req = Request::builder()
        .method("POST")
        .uri("/v1/ask")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["question"], "What is the latest Qwen model?");
    assert_eq!(json["limit"], 3);
    assert!(json["answer"].is_string());
    let context = json["context"].as_array().expect("context array");
    assert!(!context.is_empty(), "expected at least one context row");
}

#[tokio::test]
async fn ask_with_unmatched_question_returns_empty_context() {
    let app = common::seeded_app();
    let body = serde_json::json!({
        "question": "zzzzzzzzzzzzz_unlikely_term_xyzzy",
        "limit": 5,
    });
    let req = Request::builder()
        .method("POST")
        .uri("/v1/ask")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["context"].as_array().expect("array").len(), 0);
}