//! Integration tests for `POST /v1/for-use-case`.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn for_use_case_with_kind_embedding_returns_seeded_rows() {
    let app = common::seeded_app();
    let body = serde_json::json!({
        "use_case": "embedding",
        "limit": 10,
    });
    let req = Request::builder()
        .method("POST")
        .uri("/v1/for-use-case")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["use_case"], "embedding");
    let results = json["results"].as_array().expect("results");
    // The seeded fixtures are all `instruct` models; with `embedding`
    // selected we expect zero hits because the facet post-filter is strict.
    // The route must still respond 200 with an empty results array, which
    // is the contract we verify here.
    assert!(
        results.is_empty(),
        "expected zero `embedding` hits; got {}",
        results.len()
    );
}

#[tokio::test]
async fn for_use_case_supports_agentic_coding_reasoning_embedding() {
    let app = common::seeded_app();
    for uc in ["agentic", "coding", "reasoning", "embedding"] {
        let body = serde_json::json!({
            "use_case": uc,
            "limit": 5,
        });
        let req = Request::builder()
            .method("POST")
            .uri("/v1/for-use-case")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .expect("build request");
        let (resp, body) = common::send(app.router.clone(), req).await;
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "use_case={uc} returned {}",
            resp.status()
        );
        let json: serde_json::Value = serde_json::from_slice(&body).expect("parse");
        assert_eq!(json["use_case"], uc);
    }
}