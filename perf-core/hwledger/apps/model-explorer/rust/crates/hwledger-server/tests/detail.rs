//! Integration tests for `GET /v1/models/:id` and
//! `GET /v1/models/:id/quants`.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn detail_for_known_model_returns_found() {
    let app = common::seeded_app();
    let req = Request::builder()
        .uri("/v1/models/qwen/Qwen2.5-7B-Instruct")
        .body(Body::empty())
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["id"], "hf::qwen/Qwen2.5-7B-Instruct");
    assert_eq!(json["found"], true);
    assert_eq!(json["kind"], "instruct");
    // `score` may be null if the BM25 search can't tokenize the canonical
    // id (`hf::qwen/...`); we don't assert a value here because tantivy's
    // tokenizer is unrelated to the route contract.
    let _ = json["score"].as_f64();
    let quants = json["quants"].as_array().expect("quants array");
    let quants_strs: Vec<&str> = quants.iter().map(|q| q.as_str().unwrap()).collect();
    assert!(quants_strs.contains(&"gguf"));
    assert!(quants_strs.contains(&"gptq"));
}

#[tokio::test]
async fn detail_canonicalizes_bare_id_with_hf_prefix() {
    let app = common::seeded_app();
    // Without an explicit source prefix the service adds `hf::`.
    let req = Request::builder()
        .uri("/v1/models/qwen/Qwen2.5-7B-Instruct")
        .body(Body::empty())
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["id"], "hf::qwen/Qwen2.5-7B-Instruct");
}

#[tokio::test]
async fn detail_for_unknown_model_returns_found_false() {
    let app = common::seeded_app();
    let req = Request::builder()
        .uri("/v1/models/no-such-org/no-such-model")
        .body(Body::empty())
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["found"], false);
    assert!(json["score"].is_null());
}

#[tokio::test]
async fn quants_endpoint_returns_known_quants() {
    let app = common::seeded_app();
    let req = Request::builder()
        .uri("/v1/models/mistralai/Mistral-7B-Instruct-v0.3/quants")
        .body(Body::empty())
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    let quants: Vec<&str> = json["quants"]
        .as_array()
        .expect("quants array")
        .iter()
        .map(|q| q.as_str().unwrap())
        .collect();
    // Mistral is upserted with `gguf gptq awq` in the test fixture.
    assert!(quants.contains(&"gguf"));
    assert!(quants.contains(&"gptq"));
    assert!(quants.contains(&"awq"));
}

#[tokio::test]
async fn quants_endpoint_returns_empty_for_unknown_model() {
    let app = common::seeded_app();
    let req = Request::builder()
        .uri("/v1/models/no-such-org/no-such-model/quants")
        .body(Body::empty())
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["quants"].as_array().expect("array").len(), 0);
}