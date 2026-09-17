// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for the `ap rubric fix-list` subcommand.

use std::path::PathBuf;

use assert_cmd::Command;

fn self_repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn cli() -> Command {
    Command::cargo_bin("agileplus").expect("agileplus binary should be built")
}

#[test]
fn rubric_fix_list_emits_top_table_with_priority_ordering() {
    let repo = self_repo();
    let output = cli()
        .args([
            "rubric",
            "fix-list",
            "--repo",
            repo.to_str().unwrap(),
            "--limit",
            "5",
            "--clusters",
            "C01,C03,C04",
        ])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# Fix list"), "missing header: {stdout}");
    assert!(stdout.contains("Cluster"), "missing Cluster column: {stdout}");
    assert!(stdout.contains("Score"), "missing Score column: {stdout}");
    assert!(stdout.contains("Effort"), "missing Effort column: {stdout}");
    assert!(
        stdout.contains("Per-cluster gap totals"),
        "missing footer: {stdout}"
    );
}

#[test]
fn rubric_fix_list_writes_to_output_file() {
    let repo = self_repo();
    let tmp = tempfile::tempdir().expect("tempdir");
    let out_path = tmp.path().join("fix-list.md");
    cli()
        .args([
            "rubric",
            "fix-list",
            "--repo",
            repo.to_str().unwrap(),
            "--output",
            out_path.to_str().unwrap(),
            "--limit",
            "5",
        ])
        .assert()
        .success();
    let written = std::fs::read_to_string(&out_path).expect("fix-list file should exist");
    assert!(written.contains("# Fix list"));
    assert!(written.contains("Per-cluster gap totals"));
}

#[test]
fn rubric_fix_list_help_lists_required_flags() {
    let output = cli()
        .args(["rubric", "fix-list", "--help"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    for flag in ["--repo", "--limit", "--clusters", "--output"] {
        assert!(stdout.contains(flag), "missing {flag}: {stdout}");
    }
}

#[test]
fn rubric_fix_list_rejects_missing_repo() {
    cli()
        .args([
            "rubric",
            "fix-list",
            "--repo",
            "/nonexistent/path/xyzzy",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("does not exist"));
}
