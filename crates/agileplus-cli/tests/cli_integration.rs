//! Integration tests for the agileplus CLI binary.
//!
//! Uses assert_cmd to test the binary end-to-end.

use assert_cmd::Command;

#[test]
fn help_prints_usage() {
    Command::cargo_bin("agileplus")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("AgilePlus project management CLI"));
}

#[test]
fn version_flag_prints_package_version() {
    Command::cargo_bin("agileplus")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

// NOTE: The tests below (specify/research/specs/frontend/plan subcommand
// coverage) were removed here. They were merged in verbatim from a stale
// `pr-769` branch that assumed CLI subcommands — `specify`, `research`,
// `plan`, `specs`, and `frontend` — that do not exist in this crate's
// `Command` enum (see `src/main.rs`; the real surface is
// Feature/Module/Cycle/Version/Sync/SeedRequirements/ListProjects/
// ListEpics/ListStories/Trace/Dashboard/Worklog). Landing them as-is would
// have committed permanently-failing tests. This gap is the same
// aspirational-scaffolding debt already tracked and parked per PR#878;
// re-add equivalent coverage once those subcommands are actually
// implemented.
//
// Removed test fns (for reference when implementing the above):
// - specify_help_prints_subcommand_usage
// - research_help_prints_subcommand_usage
// - specify_from_file_creates_feature
// - specify_creates_spec_artifact
// - specs_audit_json_reports_legacy_gap
// - specs_audit_strict_fails_on_gap_and_passes_when_clean
// - frontend_audit_strict_accepts_active_and_marked_scaffold_surfaces
// - frontend_audit_strict_fails_on_unmarked_scaffold
// - research_on_nonexistent_feature_runs_pre_specify_mode
// - research_after_specify_transitions_to_researched
// - plan_requires_researched_state_unless_forced
// - specify_refinement_detects_no_changes
// - specify_refinement_writes_diff_artifact
// - no_git_repo_shows_helpful_error (used the "specify" subcommand)
//
// The `fixtures_dir()` and `init_temp_git_repo()` helpers those tests used
// were removed along with them; reinstate as needed alongside the real
// subcommand implementations.
