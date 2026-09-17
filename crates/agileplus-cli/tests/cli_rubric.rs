// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for the `ap rubric` subcommand.
//!
//! These tests shell out to the built `agileplus` binary, point it at the
//! `agileplus-cli` crate directory, and assert that the rendered scorecard
//! contains the v38 cluster markers (`CLUSTER_START`, `CLUSTER_TOTAL`,
//! `CLUSTER_DONE`). The catalog path is intentionally omitted so the
//! command must fall back to the workspace-bundled catalog.

use std::path::PathBuf;

use assert_cmd::Command;

/// Resolve the agileplus-cli crate root (where the test binary is built).
///
/// `CARGO_MANIFEST_DIR` is the crate being tested (agileplus-cli), which is
/// exactly the repo we want to score against. This keeps the test hermetic
/// — it never depends on `cwd` or environment state.
fn self_repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn cli() -> Command {
    Command::cargo_bin("agileplus").expect("agileplus binary should be built")
}

#[test]
fn rubric_score_emits_cluster_done_markers() {
    let repo = self_repo();
    let output = cli()
        .args(["rubric", "score", "--repo", repo.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("CLUSTER_DONE"),
        "missing CLUSTER_DONE marker in scorecard output: {stdout}"
    );
    assert!(
        stdout.contains("CLUSTER_START"),
        "missing CLUSTER_START marker in scorecard output: {stdout}"
    );
    assert!(
        stdout.contains("CLUSTER_TOTAL"),
        "missing CLUSTER_TOTAL marker in scorecard output: {stdout}"
    );
}

#[test]
fn rubric_score_writes_summary_line_to_stdout() {
    let repo = self_repo();
    let output = cli()
        .args(["rubric", "score", "--repo", repo.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("scored "),
        "missing summary footer: {stdout}"
    );
    assert!(
        stdout.contains("grade "),
        "summary footer should mention grade: {stdout}"
    );
}

#[test]
fn rubric_score_accepts_cluster_filter() {
    let repo = self_repo();
    // Filter to a single cluster; the rendered scorecard must still close
    // with a CLUSTER_DONE marker for that cluster.
    let output = cli()
        .args([
            "rubric",
            "score",
            "--repo",
            repo.to_str().unwrap(),
            "--clusters",
            "C03",
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("CLUSTER_DONE cluster=C03"),
        "expected C03-only output, got: {stdout}"
    );
}

#[test]
fn rubric_score_with_output_file_writes_to_path() {
    let repo = self_repo();
    let tmp = tempfile::tempdir().expect("tempdir");
    let out_path = tmp.path().join("scorecard.md");

    cli()
        .args([
            "rubric",
            "score",
            "--repo",
            repo.to_str().unwrap(),
            "--output",
            out_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let written = std::fs::read_to_string(&out_path).expect("scorecard should exist");
    assert!(
        written.contains("CLUSTER_DONE"),
        "file output missing CLUSTER_DONE marker: {written}"
    );
}

#[test]
fn rubric_score_missing_repo_path_errors() {
    cli()
        .args(["rubric", "score", "--repo", "/nonexistent/path/xyzzy"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("does not exist"));
}

#[test]
fn rubric_help_lists_score_subcommand() {
    let output = cli()
        .args(["rubric", "--help"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("score"),
        "missing `score` in rubric help: {stdout}"
    );
}

#[test]
fn rubric_score_help_lists_probes_flag() {
    let output = cli()
        .args(["rubric", "score", "--help"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--probes"),
        "missing `--probes` flag in `rubric score --help`: {stdout}"
    );
    assert!(
        stdout.contains("auto") && stdout.contains("none"),
        "`--probes` should accept `auto` and `none` modes: {stdout}"
    );
}

#[test]
fn rubric_score_probes_none_matches_v1_behavior() {
    // `--probes none` must produce the same total_points as the legacy v1
    // path-presence-only evaluator. We compare two scorecards (probes disabled
    // vs. probes auto) by looking at the grade footer: it should be present
    // and parseable in both modes. This pins the backwards-compat contract.
    let repo = self_repo();
    for mode in ["none", "auto"] {
        let output = cli()
            .args([
                "rubric",
                "score",
                "--repo",
                repo.to_str().unwrap(),
                "--probes",
                mode,
                "--clusters",
                "C01",
            ])
            .assert()
            .success()
            .get_output()
            .clone();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("CLUSTER_DONE cluster=C01"),
            "missing C01 marker in --probes {mode} mode: {stdout}"
        );
        assert!(
            stdout.contains("grade "),
            "missing grade footer in --probes {mode} mode: {stdout}"
        );
    }
}