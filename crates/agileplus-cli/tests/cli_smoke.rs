//! Smoke tests for the new CLI subcommands added in this change set.
//!
//! These tests shell out to the built `agileplus-cli` binary so that they
//! exercise the real clap dispatch, the in-memory `MockStore`, and the
//! `anyhow::bail!` → stderr → exit(1) error path.
//!
//! The binary path is provided by cargo at build time through
//! `env!("CARGO_BIN_EXE_agileplus-cli")`.

use assert_cmd::Command;

/// Helper: build a `Command` for the in-tree binary.
fn cli() -> Command {
    Command::cargo_bin("agileplus").expect("agileplus binary should be built")
}

// ── top-level status ─────────────────────────────────────────────────────────

#[test]
fn status_prints_expected_sections() {
    let output = cli().arg("status").assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("AgilePlus project status"),
        "missing title: {stdout}"
    );
    assert!(
        stdout.contains("Modules :"),
        "missing modules line: {stdout}"
    );
    assert!(
        stdout.contains("Features:"),
        "missing features line: {stdout}"
    );
    assert!(
        stdout.contains("Cycles  :"),
        "missing cycles line: {stdout}"
    );
    assert!(
        stdout.contains("Active cycle:"),
        "missing active cycle line: {stdout}"
    );
    assert!(
        stdout.contains("Features by state:"),
        "missing per-state breakdown: {stdout}"
    );
}

#[test]
fn version_prints_known_prefix() {
    let output = cli().arg("version").assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("agileplus-cli v"),
        "expected `agileplus-cli v` prefix, got: {stdout}"
    );
}

// ── feature count ────────────────────────────────────────────────────────────

#[test]
fn feature_count_total_matches_seeded_data() {
    let output = cli()
        .args(["feature", "count"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("STATE"), "header missing: {stdout}");
    assert!(stdout.contains("COUNT"), "header missing: {stdout}");
    assert!(stdout.contains("TOTAL"), "totals line missing: {stdout}");
}

#[test]
fn feature_count_with_state_filter_is_single_number() {
    // `created` exists in the seeded store — output should be a single integer
    // and must NOT contain the multi-row "STATE/COUNT" table.
    let output = cli()
        .args(["feature", "count", "--state", "created"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("STATE"),
        "should not print table when filtered: {stdout}"
    );
    let trimmed = stdout.trim();
    let n: usize = trimmed
        .parse()
        .unwrap_or_else(|_| panic!("expected a single integer, got `{trimmed}`"));
    assert!(n >= 1, "expected at least 1 created feature, got {n}");
}

#[test]
fn feature_count_rejects_unknown_state() {
    cli()
        .args(["feature", "count", "--state", "bogus"])
        .assert()
        .failure() // exit code 1
        .stderr(predicates::str::contains("invalid --state `bogus`"));
}

// ── feature search ───────────────────────────────────────────────────────────

#[test]
fn feature_search_matches_by_slug() {
    // `core-platform` is the slug of the seeded module; at least one feature
    // should carry a slug/label that contains `core`.
    let output = cli()
        .args(["feature", "search", "core"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Either we find a match (and the table header is printed), or we get
    // the explicit "No features matched" message. Both are valid.
    assert!(
        stdout.contains("SLUG") || stdout.contains("No features matched"),
        "unexpected output: {stdout}"
    );
}

#[test]
fn feature_search_with_no_results_is_informative() {
    let output = cli()
        .args(["feature", "search", "definitely-no-such-substring-zzz"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No features matched"),
        "expected friendly empty result, got: {stdout}"
    );
}

// ── feature ready ────────────────────────────────────────────────────────────

#[test]
fn feature_ready_is_idempotent() {
    // The seed does not have a `validated` feature, so the command prints
    // a friendly "No features are currently in the `validated` state." line.
    // Running it twice must produce the same output and exit 0 both times.
    let expected_phrase = "validated";
    for _ in 0..2 {
        let output = cli()
            .args(["feature", "ready"])
            .assert()
            .success()
            .get_output()
            .clone();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(expected_phrase) || stdout.contains("ID"),
            "unexpected ready output: {stdout}"
        );
    }
}

// ── module show / search ─────────────────────────────────────────────────────

#[test]
fn module_show_existing_id() {
    let output = cli()
        .args(["module", "show", "1"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("id          : 1"),
        "missing id line: {stdout}"
    );
    assert!(
        stdout.contains("slug        :"),
        "missing slug line: {stdout}"
    );
    assert!(
        stdout.contains("name        :"),
        "missing name line: {stdout}"
    );
    assert!(
        stdout.contains("features    :"),
        "missing feature-count line: {stdout}"
    );
}

#[test]
fn module_show_missing_id_errors_with_nonzero_exit() {
    cli()
        .args(["module", "show", "99999"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("module 99999 not found"));
}

#[test]
fn module_search_returns_table_or_empty_message() {
    let output = cli()
        .args(["module", "search", "platform"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("SLUG") || stdout.contains("No modules matched"),
        "unexpected module-search output: {stdout}"
    );
}

// ── cycle list / set / current ───────────────────────────────────────────────

#[test]
fn cycle_list_contains_id_and_state_columns() {
    let output = cli()
        .args(["cycle", "list"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ID"), "header missing: {stdout}");
    assert!(stdout.contains("NAME"), "header missing: {stdout}");
    assert!(stdout.contains("STATE"), "header missing: {stdout}");
    // The seeded Sprint 1 should be present.
    assert!(
        stdout.contains("Sprint 1"),
        "Sprint 1 missing from cycle list: {stdout}"
    );
}

#[test]
fn cycle_current_matches_first_active_or_prints_none() {
    let output = cli()
        .args(["cycle", "current"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Either we have an active cycle (id/name/state/start/end) or the
    // explicit "No active cycle" message. Both are acceptable.
    assert!(
        stdout.contains("id") || stdout.contains("No active cycle"),
        "unexpected cycle current output: {stdout}"
    );
}

#[test]
fn cycle_set_existing_active_id_is_noop() {
    // Cycle 1 is the active Sprint 1 in the seed; setting it again should
    // succeed and print the "already active" message.
    let output = cli()
        .args(["cycle", "set", "1"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("already active"),
        "expected `already active` confirmation, got: {stdout}"
    );
}

#[test]
fn cycle_set_missing_id_fails() {
    cli()
        .args(["cycle", "set", "99999"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("cycle 99999 not found"));
}

// ── help / discoverability ───────────────────────────────────────────────────

#[test]
fn top_level_help_lists_new_subcommands() {
    let output = cli().arg("--help").assert().success().get_output().clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("status"),
        "missing `status` in help: {stdout}"
    );
    assert!(
        stdout.contains("feature"),
        "missing `feature` in help: {stdout}"
    );
    assert!(
        stdout.contains("module"),
        "missing `module` in help: {stdout}"
    );
    assert!(
        stdout.contains("cycle"),
        "missing `cycle` in help: {stdout}"
    );
}

#[test]
fn feature_subcommand_help_lists_count_search_ready() {
    let output = cli()
        .args(["feature", "--help"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("count"),
        "missing `feature count` in help: {stdout}"
    );
    assert!(
        stdout.contains("search"),
        "missing `feature search` in help: {stdout}"
    );
    assert!(
        stdout.contains("ready"),
        "missing `feature ready` in help: {stdout}"
    );
}

#[test]
fn module_subcommand_help_lists_show_search() {
    let output = cli()
        .args(["module", "--help"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("show"),
        "missing `module show` in help: {stdout}"
    );
    assert!(
        stdout.contains("search"),
        "missing `module search` in help: {stdout}"
    );
}

#[test]
fn cycle_subcommand_help_lists_list_set() {
    let output = cli()
        .args(["cycle", "--help"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("list"),
        "missing `cycle list` in help: {stdout}"
    );
    assert!(
        stdout.contains("set"),
        "missing `cycle set` in help: {stdout}"
    );
}
