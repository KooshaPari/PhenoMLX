//! End-to-end tests for the MCP server against a real tantivy fixture.
//!
//! These tests bypass the JSON-RPC transport and exercise the
//! [`ServiceBackend`] directly so we can assert on the JSON shape each
//! tool returns without first stringifying through a `tools/call`
//! envelope. They mirror the `seeded_app` fixture in
//! `hwledger-server/tests/common.rs` — three real model rows, hand-rolled
//! into the store, then queried through the same service functions the
//! MCP tools delegate to.
//!
//! Tests covering the JSON-RPC envelope itself (the `tools/call`
//! serializer, the dispatcher's argument validation, etc.) live as
//! unit tests in `transport.rs` and `tools.rs` and use a `MockBackend`
//! so they don't need a tantivy instance.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use hwledger_mcp::backend::{Backend, ServiceBackend};
use hwledger_search_index::{upsert_model, IndexedModel, TantivyStore};
use serde_json::json;
use tempfile::TempDir;

/// Three-row tantivy fixture used by every test in this file.
///
/// Returns the opened store and the temp dir that backs it. Holding
/// the dir until the end of the test keeps tantivy's on-disk segments
/// alive for the store's lifetime.
fn seeded_store() -> (TempDir, Arc<TantivyStore>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TantivyStore::open(dir.path()).expect("open tantivy");
    upsert_model(
        &store,
        &IndexedModel {
            id: "hf::qwen/Qwen2.5-7B-Instruct".into(),
            name: "Qwen2.5 7B Instruct".into(),
            org: "qwen".into(),
            kind: "instruct".into(),
            family: "qwen2".into(),
            arch: "gqa".into(),
            quants: vec!["gguf".into(), "gptq".into()],
            card_snippet: "Qwen2.5 is the latest series of large language models from Alibaba."
                .into(),
        },
    )
    .expect("upsert qwen");
    upsert_model(
        &store,
        &IndexedModel {
            id: "hf::meta-llama/Llama-3-8B-Instruct".into(),
            name: "Llama 3 8B Instruct".into(),
            org: "meta-llama".into(),
            kind: "instruct".into(),
            family: "llama".into(),
            arch: "gqa".into(),
            quants: vec!["gguf".into()],
            card_snippet: "Meta's Llama 3 instruction-tuned model.".into(),
        },
    )
    .expect("upsert llama");
    upsert_model(
        &store,
        &IndexedModel {
            id: "hf::mistralai/Mistral-7B-Instruct-v0.3".into(),
            name: "Mistral 7B Instruct v0.3".into(),
            org: "mistralai".into(),
            kind: "instruct".into(),
            family: "mistral".into(),
            arch: "sma".into(),
            quants: vec!["gguf".into(), "gptq".into(), "awq".into()],
            card_snippet: "Mistral 7B base fine-tuned for instruction following.".into(),
        },
    )
    .expect("upsert mistral");
    store.commit().expect("commit");
    (dir, Arc::new(store))
}

#[test]
fn model_search_returns_results_with_expected_shape() {
    let (_dir, store) = seeded_store();
    let backend = ServiceBackend::new(store).expect("ServiceBackend::new");

    let value = backend
        .model_search(&json!({"query": "qwen instruct", "limit": 5}))
        .expect("model_search");

    let obj = value.as_object().expect("model_search result is an object");
    assert_eq!(obj["query"], json!("qwen instruct"));
    assert_eq!(obj["limit"], json!(5));
    assert!(
        obj["total"].as_u64().expect("total is a number") >= 1,
        "fixture should match qwen, got total={}",
        obj["total"]
    );
    let results = obj["results"].as_array().expect("results is an array");
    assert!(!results.is_empty(), "results should be non-empty");
    let first = &results[0];
    // Each FusedResult carries an `id` and a `score`; the schema is
    // owned by `hwledger-search-core` so we only assert on the public
    // shape rather than the field order.
    assert!(first.get("id").is_some(), "result missing `id`");
    assert!(first.get("score").is_some(), "result missing `score`");
}

#[test]
fn model_search_filters_by_kind_facet() {
    let (_dir, store) = seeded_store();
    let backend = ServiceBackend::new(store).expect("ServiceBackend::new");

    // All three fixture rows are `instruct`, so a kinds=[instruct] filter
    // returns them; a kinds=[embedding] filter returns none. The
    // backend should silently drop unknown kinds rather than error.
    //
    // We don't pin the exact total for the unrestricted search because
    // BM25 relevance scoring depends on the tokenised query and the
    // fixture's card_snippet length — what matters for this test is
    // that (a) the unrestricted search returns at least one row and
    // (b) the embedding filter returns zero.
    let all = backend
        .model_search(&json!({"query": "model", "limit": 10}))
        .expect("all");
    let embedding_only = backend
        .model_search(&json!({"query": "model", "limit": 10, "kinds": ["embedding"]}))
        .expect("embedding");

    assert!(
        all["total"].as_u64().unwrap() >= 1,
        "fixture should match at least one row"
    );
    assert_eq!(
        embedding_only["total"].as_u64().unwrap(),
        0,
        "no embedding rows in fixture"
    );
}

#[test]
fn model_detail_finds_canonical_id_and_returns_envelope() {
    let (_dir, store) = seeded_store();
    let backend = ServiceBackend::new(store).expect("ServiceBackend::new");

    let value = backend
        .model_detail(&json!({"source": "hf", "id": "qwen/Qwen2.5-7B-Instruct"}))
        .expect("model_detail");

    // The tool prepends `{source}::` so the canonical id is `hf::qwen/...`.
    assert_eq!(value["canonical_id"], json!("hf::qwen/Qwen2.5-7B-Instruct"));
    assert_eq!(value["source"], json!("hf"));
    assert_eq!(value["id"], json!("qwen/Qwen2.5-7B-Instruct"));

    let detail = value["detail"].as_object().expect("detail is an object");
    assert_eq!(detail["found"], json!(true));
    assert_eq!(detail["id"], json!("hf::qwen/Qwen2.5-7B-Instruct"));
    assert_eq!(detail["kind"], json!("instruct"));
}

#[test]
fn model_detail_reports_found_false_for_unknown_id() {
    let (_dir, store) = seeded_store();
    let backend = ServiceBackend::new(store).expect("ServiceBackend::new");

    let value = backend
        .model_detail(&json!({"source": "hf", "id": "no/such-model"}))
        .expect("model_detail");

    // detail_for_id is infallible — it returns `found: false` instead of
    // erroring, so the LLM client can present a "no result" branch.
    let detail = value["detail"].as_object().unwrap();
    assert_eq!(detail["found"], json!(false));
}

#[test]
fn model_quants_returns_recorded_quantization_list() {
    let (_dir, store) = seeded_store();
    let backend = ServiceBackend::new(store).expect("ServiceBackend::new");

    let value = backend
        .model_quants(&json!({"source": "hf", "id": "mistralai/Mistral-7B-Instruct-v0.3"}))
        .expect("model_quants");

    let quants = value["quants"].as_array().expect("quants is an array");
    let strings: Vec<&str> = quants.iter().filter_map(|v| v.as_str()).collect();
    // The fixture row tags mistral with three formats; quants_for_id
    // surfaces them in insertion order.
    assert!(
        strings.contains(&"gguf") && strings.contains(&"gptq") && strings.contains(&"awq"),
        "expected gguf + gptq + awq, got {strings:?}"
    );
}

#[test]
fn similar_models_excludes_the_seed_and_uses_its_id_as_query() {
    let (_dir, store) = seeded_store();
    let backend = ServiceBackend::new(store).expect("ServiceBackend::new");

    let value = backend
        .similar_models(&json!({"source": "hf", "id": "qwen/Qwen2.5-7B-Instruct", "limit": 10}))
        .expect("similar_models");

    assert_eq!(value["seed"], json!("hf::qwen/Qwen2.5-7B-Instruct"));
    assert_eq!(value["limit"], json!(10));
    let results = value["results"].as_array().unwrap();
    for r in results {
        assert_ne!(
            r["id"], "hf::qwen/Qwen2.5-7B-Instruct",
            "seed must be filtered out of similar_models results"
        );
    }
}

#[test]
fn models_for_use_case_accepts_known_use_case_strings() {
    let (_dir, store) = seeded_store();
    let backend = ServiceBackend::new(store).expect("ServiceBackend::new");

    // "coding" maps to ModelKind::Coding; the fixture has no coding rows
    // so total should be zero. The point of the test is the dispatch /
    // parsing plumbing, not the ranking.
    let value = backend
        .models_for_use_case(&json!({"use_case": "coding", "limit": 10}))
        .expect("models_for_use_case");

    assert_eq!(value["use_case"], json!("coding"));
    assert_eq!(value["limit"], json!(10));
    assert_eq!(value["total"].as_u64().unwrap(), 0);
}

#[test]
fn models_for_use_case_rejects_unknown_use_case() {
    let (_dir, store) = seeded_store();
    let backend = ServiceBackend::new(store).expect("ServiceBackend::new");

    let err = backend
        .models_for_use_case(&json!({"use_case": "nope"}))
        .expect_err("unknown use case must error");
    // The error is `InvalidParams` (the JSON-RPC layer surfaces this as
    // -32602 so the LLM client can see the enum constraint).
    assert_eq!(err.code(), -32602);
}

#[test]
fn model_rag_ask_returns_stub_answer_with_top_k_context() {
    let (_dir, store) = seeded_store();
    let backend = ServiceBackend::new(store).expect("ServiceBackend::new");

    let value = backend
        .model_rag_ask(&json!({"question": "what's a good instruct model", "top_k": 3}))
        .expect("model_rag_ask");

    assert_eq!(value["question"], json!("what's a good instruct model"));
    assert_eq!(value["limit"], json!(3));
    let context = value["context"].as_array().expect("context is an array");
    assert!(context.len() <= 3, "context should be bounded by top_k");
    // The v1 stub message is deterministic; the data-pipeline-driven
    // answer will land later.
    let answer = value["answer"].as_str().expect("answer is a string");
    assert!(
        answer.contains("stub"),
        "answer should carry the v1 stub marker, got `{answer}`"
    );
}

#[test]
fn missing_required_argument_yields_invalid_params_error() {
    let (_dir, store) = seeded_store();
    let backend = ServiceBackend::new(store).expect("ServiceBackend::new");

    // The backend's own arg validation kicks in even if the dispatcher
    // misses something — second line of defence against malformed calls.
    let err = backend
        .model_detail(&json!({"source": "hf"}))
        .expect_err("missing `id`");
    assert_eq!(err.code(), -32602);

    let err = backend
        .model_search(&json!({"limit": 5}))
        .expect_err("missing `query`");
    assert_eq!(err.code(), -32602);
}
