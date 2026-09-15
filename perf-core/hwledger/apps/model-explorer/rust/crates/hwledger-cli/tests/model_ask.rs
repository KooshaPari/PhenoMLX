//! Integration tests for the `model-ask` top-level subcommand.

use std::process::Command;

use assert_cmd::prelude::*;
use hwledger_search_index::{upsert_model, IndexedModel, TantivyStore};
use predicates::prelude::*;
use tempfile::TempDir;

/// Build a populated tantivy store at `dir`. Drops the handle before
/// returning so the spawned CLI can re-open the same directory without
/// tripping tantivy's writer lock.
fn build_fixture_index(dir: &std::path::Path) {
    let store = TantivyStore::open(dir).expect("open store");
    let fixtures = vec![
        (
            "hf::qwen/Qwen2.5-7B-Instruct",
            "Qwen2.5-7B-Instruct",
            "qwen",
            "instruct",
        ),
        (
            "hf::meta-llama/Llama-3.1-8B-Instruct",
            "Llama-3.1-8B-Instruct",
            "meta-llama",
            "instruct",
        ),
        (
            "hf::deepseek-ai/DeepSeek-V3-Base",
            "DeepSeek-V3-Base",
            "deepseek-ai",
            "base",
        ),
    ];
    for (id, name, org, kind) in fixtures {
        let model = IndexedModel {
            id: id.to_string(),
            name: name.to_string(),
            org: org.to_string(),
            kind: kind.to_string(),
            family: "unknown".to_string(),
            arch: "unknown".to_string(),
            quants: vec!["safetensors".to_string()],
            card_snippet: format!("{name} is a {kind} model"),
        }
        .truncated();
        upsert_model(&store, &model).expect("upsert");
    }
    store.commit().expect("commit");
    drop(store);
}

fn cli_bin() -> Command {
    Command::cargo_bin("hwledger-explorer-cli").expect("binary built by cargo")
}

#[test]
fn model_ask_human_returns_answer_and_table() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    cli_bin()
        .args([
            "--index",
            dir.path().to_str().unwrap(),
            "model-ask",
            "Qwen",
        ])
        .assert()
        .success()
        // The v1 stub returns a "(stub)" prefix on every answer.
        .stdout(predicate::str::contains("(stub)"));
}

#[test]
fn model_ask_json_emits_question_and_context() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    let out: Vec<u8> = cli_bin()
        .args([
            "--index",
            dir.path().to_str().unwrap(),
            "--json",
            "model-ask",
            "Llama",
            "--limit",
            "3",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("json parse");
    assert_eq!(v["question"], "Llama");
    assert_eq!(v["limit"], 3);
    assert!(v["answer"].as_str().unwrap().contains("Llama"));
    assert!(v["context"].is_array());
}

#[test]
fn model_ask_empty_query_returns_empty_context() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    // Empty text is intentionally mapped to "no results" inside run_hybrid;
    // the answer stub is still emitted, but the context array is empty.
    let out: Vec<u8> = cli_bin()
        .args([
            "--index",
            dir.path().to_str().unwrap(),
            "--json",
            "model-ask",
            "   ",
            "--limit",
            "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("json parse");
    let ctx = v["context"].as_array().unwrap();
    assert_eq!(ctx.len(), 0);
}
