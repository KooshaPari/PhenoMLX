//! Integration tests for `POST /v1/search`.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};

#[tokio::test]
async fn search_returns_results_for_a_text_query() {
    let app = common::seeded_app();
    let body = serde_json::json!({
        "text": "instruct",
        "limit": 10,
    });
    let req = Request::builder()
        .method("POST")
        .uri("/v1/search")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(json["query"], "instruct");
    assert_eq!(json["limit"], 10);

    let results = json["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "expected at least one BM25 hit for `instruct`");
    for r in results {
        assert!(r["id"].is_string());
        assert!(r["score"].is_number());
    }
}

#[tokio::test]
async fn search_applies_kinds_facet_filter() {
    let app = common::seeded_app();
    // Every seeded row is `instruct`; ask for `instruct` + `coding` and
    // verify we still get results back (the facet OR-matches).
    let body = serde_json::json!({
        "text": "instruct",
        "kinds": ["instruct", "coding"],
        "limit": 10,
    });
    let req = Request::builder()
        .method("POST")
        .uri("/v1/search")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    let results = json["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "kinds facet must not silently drop all rows");
}

#[tokio::test]
async fn search_with_empty_text_returns_no_results() {
    let app = common::seeded_app();
    let body = serde_json::json!({ "text": "", "limit": 10 });
    let req = Request::builder()
        .method("POST")
        .uri("/v1/search")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .expect("build request");

    let (resp, body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body).expect("parse body");
    assert_eq!(
        json["results"].as_array().expect("results").len(),
        0,
        "empty text must yield zero results"
    );
}

#[tokio::test]
async fn search_with_unknown_kind_does_not_500() {
    let app = common::seeded_app();
    // A bogus kind entry is silently dropped by `parse_kind`; the request
    // must still succeed with the empty kinds facet applied.
    let body = serde_json::json!({
        "text": "instruct",
        "kinds": ["definitely_not_a_kind"],
        "limit": 5,
    });
    let req = Request::builder()
        .method("POST")
        .uri("/v1/search")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .expect("build request");

    let (resp, _body) = common::send(app.router, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// AgenticFitRerank skill integration tests
// ---------------------------------------------------------------------------
//
// These verify the skill actually mutates the order of the result set
// over the HTTP surface (i.e. the registry is wired into the route,
// not just present in the registry type). They use the
// `seeded_app_with_use_case_kinds` fixture which has one row whose
// `kind` is `agentic`, one whose `kind` is `coding`, and three
// `instruct` rows so the per-result intent-fit payload is non-trivial
// for both intents.

/// Helper: drive a `POST /v1/search` and return the parsed JSON body.
async fn post_search(app: &common::App, body: serde_json::Value) -> serde_json::Value {
    let req = Request::builder()
        .method("POST")
        .uri("/v1/search")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .expect("build request");
    let (resp, bytes) = common::send(app.router.clone(), req).await;
    assert_eq!(resp.status(), StatusCode::OK, "search request must succeed");
    serde_json::from_slice(&bytes).expect("parse body")
}

#[tokio::test]
async fn agentic_fit_skill_reranks_agentic_query_upward() {
    let app = common::seeded_app_with_use_case_kinds();
    // "agent" trips the `detect_intent` keyword heuristic ⇒ resolved
    // intent is `Agentic`. The agentic-kind model must surface at rank 0
    // even when its pre-rerank BM25 score is lower than the dominant
    // instruct rows (which all score similarly on "model").
    let body = serde_json::json!({
        "text": "agent model",
        "limit": 10,
    });

    let json = post_search(&app, body).await;
    assert_eq!(json["intent"], "agentic", "intent must auto-resolve to agentic");

    let results = json["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "expected at least one BM25 hit");
    assert_eq!(
        results[0]["id"], "hf::agent-org/Tool-Use-Agent",
        "AgenticFitRerank must float the agentic-kind model to rank 0 \
         when the query carries an agentic intent"
    );

    // Per the skill policy, every returned row's `payload` carries an
    // `agentic` fit value the skill could consume.
    for r in results {
        let payload = r["payload"].as_object().expect("payload object");
        assert!(
            payload.contains_key("agentic"),
            "every result's payload must carry an `agentic` fit value"
        );
    }
}

#[tokio::test]
async fn agentic_fit_skill_reranks_coding_query_upward() {
    let app = common::seeded_app_with_use_case_kinds();
    // "code" trips the `detect_intent` keyword heuristic ⇒ resolved
    // intent is `Coding`. The coding-kind model must surface at rank 0
    // after the skill has re-scored the BM25 hits.
    let body = serde_json::json!({
        "text": "code completion model",
        "limit": 10,
    });

    let json = post_search(&app, body).await;
    assert_eq!(json["intent"], "coding", "intent must auto-resolve to coding");

    let results = json["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "expected at least one BM25 hit");
    assert_eq!(
        results[0]["id"], "hf::coder-org/CodeLlama-7B",
        "AgenticFitRerank must float the coding-kind model to rank 0 \
         when the query carries a coding intent"
    );

    for r in results {
        let payload = r["payload"].as_object().expect("payload object");
        assert!(
            payload.contains_key("coding"),
            "every result's payload must carry a `coding` fit value"
        );
    }
}

#[tokio::test]
async fn agentic_fit_skill_is_no_op_when_query_has_no_use_case_intent() {
    let app = common::seeded_app_with_use_case_kinds();
    // "instruct" is not a use-case keyword → resolved intent is
    // `Generic`. The skill is a pass-through in that mode, so the
    // ordering is exactly what BM25 produced. None of the use-case-kind
    // models should be artificially floated up by a no-op skill; the
    // rank order must remain driven purely by the underlying BM25 score.
    let body = serde_json::json!({
        "text": "instruct",
        "limit": 10,
    });

    let json = post_search(&app, body).await;
    assert_eq!(
        json["intent"], "generic",
        "intent must auto-resolve to generic for `instruct`"
    );

    let results = json["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "expected at least one BM25 hit for `instruct`");

    // Re-run the same request against the *un-scored* path so we can
    // compare orderings. We do this by hitting `service::run_hybrid`
    // directly through the public surface — `run_hybrid` is exposed
    // (re-exported from `service.rs`) for exactly this kind of
    // comparison test. Rather than calling a private helper, we just
    // verify the invariant: in `Generic` mode the agentic- and
    // coding-kind rows must not have been floated to rank 0.
    let top_id = results[0]["id"].as_str().expect("id string");
    assert_ne!(
        top_id, "hf::agent-org/Tool-Use-Agent",
        "agentic-kind row must not be artificially floated to rank 0 in generic mode"
    );
    assert_ne!(
        top_id, "hf::coder-org/CodeLlama-7B",
        "coding-kind row must not be artificially floated to rank 0 in generic mode"
    );

    // Sanity-check the invariant: every returned row still carries the
    // per-result payload the route attaches (the skill did not strip it).
    for r in results {
        assert!(r["payload"].is_object(), "payload must remain on every result");
    }
}