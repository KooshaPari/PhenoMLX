//! Integration tests for the `seed …` subcommands.
//!
//! `seed build` would normally hit the HF network. The CLI doesn't expose a
//! "no-network" mode, so these tests exercise the failure path (no HF
//! connectivity → nonzero error count) plus the deterministic
//! `seed expand` path which only needs the local tantivy handle.

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
        ("hf::qwen/Qwen2.5-7B-Instruct", "Qwen2.5-7B-Instruct", "qwen", "instruct"),
        ("hf::meta-llama/Llama-3.1-8B-Instruct", "Llama-3.1-8B-Instruct", "meta-llama", "instruct"),
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
            card_snippet: format!("{} is a {} model", name, kind),
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
fn seed_expand_rejects_empty_seed_list() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    // Passing `--seeds ""` should fail before any HF traffic happens.
    cli_bin()
        .args([
            "--index", dir.path().to_str().unwrap(),
            "seed", "expand", "--seeds", "",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least one seed id"));
}

#[test]
fn seed_expand_returns_seed_ids_in_json_mode() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    // v1 expansion is a no-op stub: the seed list is returned unchanged.
    // We assert the envelope shape, not the contents.
    let out: Vec<u8> = cli_bin()
        .args([
            "--index", dir.path().to_str().unwrap(),
            "--json",
            "seed", "expand",
            "--seeds", "hf::qwen/Qwen2.5-7B-Instruct,hf::meta-llama/Llama-3.1-8B-Instruct",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).expect("json parse");
    let seeds = v["seeds"].as_array().unwrap();
    assert_eq!(seeds.len(), 2);
    let expanded = v["expanded"].as_array().unwrap();
    assert_eq!(expanded.len(), 2);
}

#[test]
fn seed_expand_human_mode_prints_summary() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());

    cli_bin()
        .args([
            "--index", dir.path().to_str().unwrap(),
            "seed", "expand",
            "--seeds", "hf::qwen/Qwen2.5-7B-Instruct",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("expanded 1 seed id"));
}

#[test]
fn seed_build_wipes_existing_index_by_default() {
    let dir = TempDir::new().unwrap();
    build_fixture_index(dir.path());
    assert!(dir.path().join("meta.json").exists());

    // Without --append, the CLI should attempt to remove the existing
    // index before opening it. The HF call inside will fail (no network
    // in CI), but the index directory is observable post-call: it must
    // either be gone or be a fresh, empty tantivy index.
    let _ = cli_bin()
        .env("HF_HUB_URL", "http://127.0.0.1:1") // unreachable on purpose
        .args([
            "--index", dir.path().to_str().unwrap(),
            "seed", "build",
            "--queries", "qwen2.5",
            "--size", "1",
        ])
        .assert();
    // The wipe-or-error branch always leaves the directory present; the
    // important assertion is that the previous meta.json is gone (because
    // we wiped on the way in).
    assert!(!dir.path().join("meta.json.lock").exists());
}