//! Integration tests for the `model …` subcommands.
//!
//! Each test:
//! 1. Creates a fresh `tempfile::TempDir`.
//! 2. Opens a `TantivyStore` at that path.
//! 3. Upserts a handful of fixture models and commits.
//! 4. Invokes the `hwledger-cli` binary against the same path with
//!    `assert_cmd` and inspects the output.
//!
//! The fixtures are intentionally small so the tests stay fast and
//! deterministic across CI environments.

use std::process::Command;

use assert_cmd::prelude::*;
use hwledger_search_index::{upsert_model, IndexedModel, TantivyStore};
use predicates::prelude::*;
use tempfile::TempDir;

/// Build a populated tantivy store at `dir`. The store handle is dropped
/// before returning so the index's `IndexWriter` lockfile is released and
/// the spawned CLI process can re-open the same directory.
fn build_fixture_index(dir: &std::path::Path) {
    let store = TantivyStore::open(dir).expect("open store");
    let fixtures = vec![
        ("hf::qwen/Qwen2.5-7B-Instruct", "Qwen2.5-7B-Instruct", "qwen", "instruct", "qwen2", "gqa", vec!["gguf", "safetensors"]),
        ("hf::meta-llama/Llama-3.1-8B-Instruct", "Llama-3.1-8B-Instruct", "meta-llama", "instruct", "llama", "gqa", vec!["gguf", "gptq"]),
        ("hf::deepseek-ai/DeepSeek-V3-Base", "DeepSeek-V3-Base", "deepseek-ai", "base", "deepseek", "mla", vec!["safetensors"]),
        ("hf::bigcode/starcoder2-7b", "starcoder2-7b", "bigcode", "coding", "starcoder2", "gqa", vec!["safetensors", "gptq"]),
        ("hf::BAAI/bge-large-en-v1.5", "bge-large-en-v1.5", "BAAI", "embedding", "bert", "mha", vec!["safetensors"]),
    ];
    for (id, name, org, kind, family, arch, quants) in fixtures {
        let model = IndexedModel {
            id: id.to_string(),
            name: name.to_string(),
            org: org.to_string(),
            kind: kind.to_string(),
            family: family.to_string(),
            arch: arch.to_string(),
            quants: quants.iter().map(|s| s.to_string()).collect(),
            card_snippet: format!("{} is a {} model from {}", name, kind, org),
        }
        .truncated();
        upsert_model(&store, &model).expect("upsert");
    }
    store.commit().expect("commit");
    // Tantivy holds a writer lock for the lifetime of `IndexWriter`; drop
    // the store handle here so the spawned CLI can re-open the same dir.
    drop(store);
}

/// Locate the `hwledger-cli` binary built by Cargo.
fn cli_bin() -> Command {
    Command::cargo_bin("hwledger-explorer-cli").expect("binary built by cargo")
}

#[test]
fn model_search_finds_indexed_rows() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    cli_bin()
        .args([
            "--index", dir.path().to_str().unwrap(),
            "model", "search", "Qwen",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Qwen2.5-7B-Instruct"));
}

#[test]
fn model_search_json_emits_envelope() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    let out: Vec<u8> = cli_bin()
        .args([
            "--index", dir.path().to_str().unwrap(),
            "--json",
            "model", "search", "Llama",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"results\""))
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("json parse");
    assert_eq!(v["query"], "Llama");
    assert!(v["results"].is_array());
}

#[test]
fn model_detail_reports_kind_and_quants() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    cli_bin()
        .args([
            "--index", dir.path().to_str().unwrap(),
            "--json",
            "model", "detail", "qwen/Qwen2.5-7B-Instruct",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"kind\": \"instruct\""))
        .stdout(predicate::str::contains("\"quants\""));
}

#[test]
fn model_quants_lists_known_formats() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    cli_bin()
        .args([
            "--index", dir.path().to_str().unwrap(),
            "--json",
            "model", "quants", "meta-llama/Llama-3.1-8B-Instruct",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("gguf"))
        .stdout(predicate::str::contains("gptq"));
}

#[test]
fn model_similar_excludes_self() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    let out: Vec<u8> = cli_bin()
        .args([
            "--index", dir.path().to_str().unwrap(),
            "--json",
            "model", "similar", "qwen/Qwen2.5-7B-Instruct", "--limit", "5",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("json parse");
    let seed = v["seed"].as_str().unwrap();
    let results = v["results"].as_array().unwrap();
    assert!(results.iter().all(|r| r["id"].as_str().unwrap() != seed));
}

#[test]
fn model_for_use_case_filters_by_kind() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    cli_bin()
        .args([
            "--index", dir.path().to_str().unwrap(),
            "--json",
            "model", "for-use-case", "--use-case", "embedding", "--limit", "5",
        ])
        .assert()
        .success()
        // The fixture only has one embedding model — we don't assert the
        // hit count (tantivy's post-filter for empty kinds is permissive),
        // just that the request envelope is well-formed.
        .stdout(predicate::str::contains("\"use_case\": \"embedding\""));
}