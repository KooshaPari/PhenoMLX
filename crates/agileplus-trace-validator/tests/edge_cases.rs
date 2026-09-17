// SPDX-License-Identifier: MIT OR Apache-2.0
use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

/// Edge case: an empty `traces/` directory should validate cleanly with zero traces.
#[test]
fn validate_empty_traces_directory_succeeds() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    fs::write(repo.path().join("FUNCTIONAL_REQUIREMENTS.md"), "").unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["validate", repo.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("validated 0 trace files"));
}

/// Edge case: a malformed JSON trace file should cause the validator to fail
/// rather than panic or silently ignore the file.
#[test]
fn validate_malformed_json_trace_fails() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    // Truncated JSON â€” not valid syntax and missing required fields.
    fs::write(
        repo.path().join("traces/FR-broken.json"),
        r##"{ "fr_id": "FR-broken", "spec_slug": "##,
    )
    .unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["validate", repo.path().to_str().unwrap()])
        .assert()
        .failure();
}

/// Edge case: an empty payload (zero-byte JSON file) should fail validation
/// rather than panic or be silently accepted.
#[test]
fn validate_empty_payload_fails() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    fs::write(repo.path().join("traces/FR-empty.json"), "").unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["validate", repo.path().to_str().unwrap()])
        .assert()
        .failure();
}

/// Edge case: a JSON document missing a required string field should fail
/// schema validation with a clear error (not crash on deserialization).
#[test]
fn validate_trace_missing_required_field_fails() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    // Valid JSON syntax but missing the `fr_id` field the schema requires.
    fs::write(
        repo.path().join("traces/FR-bad.json"),
        r##"{
  "spec_slug": "eco-024-traceability",
  "spec_anchor": "#fr-bad",
  "docs_pages": [],
  "tests": [],
  "code_modules": [],
  "journeys": []
}"##,
    )
    .unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["validate", repo.path().to_str().unwrap()])
        .assert()
        .failure();
}

/// Edge case: a single trace carrying many links across every category
/// (docs, tests, code, journeys) should validate and report the full
/// reference count in stats output.
#[test]
fn validate_trace_with_many_links_succeeds() {
    let repo = TempDir::new().unwrap();
    fs::create_dir_all(repo.path().join("traces")).unwrap();
    fs::create_dir_all(repo.path().join("docs/operations/journeys")).unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();

    for path in [
        "docs/traceability.md",
        "docs/architecture.md",
        "docs/operations/journeys/FR-9.md",
        "src/lib.rs",
        "src/main.rs",
    ] {
        if let Some(parent) = std::path::Path::new(path).parent() {
            fs::create_dir_all(repo.path().join(parent)).unwrap();
        }
        fs::write(repo.path().join(path), "stub").unwrap();
    }
    fs::write(repo.path().join("FUNCTIONAL_REQUIREMENTS.md"), "- FR-9").unwrap();
    fs::write(
        repo.path().join("traces/FR-9.json"),
        r##"{
  "fr_id": "FR-9",
  "spec_slug": "eco-024-traceability",
  "spec_anchor": "#fr-9",
  "docs_pages": ["docs/traceability.md", "docs/architecture.md"],
  "tests": ["tests/cli.rs::validate_trace_with_many_links_succeeds"],
  "code_modules": ["src/lib.rs", "src/main.rs"],
  "journeys": ["docs/operations/journeys/FR-9.md"]
}"##,
    )
    .unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["stats", repo.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("traces: 1"))
        .stdout(contains("references: 6"));
}

/// Edge case: multiple traces should all be counted in stats output.
#[test]
fn stats_reports_multiple_traces() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::create_dir_all(repo.path().join("docs/operations/journeys")).unwrap();
    fs::write(repo.path().join("docs/traceability.md"), "trace docs").unwrap();

    for (i, fr) in ["FR-1", "FR-2", "FR-3"].iter().enumerate() {
        fs::write(repo.path().join("src/module.rs"), "fn x() {}").unwrap();
        fs::write(
            repo.path()
                .join(format!("docs/operations/journeys/{fr}.md")),
            "journey",
        )
        .unwrap();
        fs::write(
            repo.path().join(format!("traces/{fr}.json")),
            format!(
                r##"{{
  "fr_id": "{fr}",
  "spec_slug": "eco-{i:03}-traceability",
  "spec_anchor": "#{fr}",
  "docs_pages": ["docs/traceability.md"],
  "tests": ["tests/cli.rs::stats_reports_multiple_traces"],
  "code_modules": ["src/module.rs"],
  "journeys": ["docs/operations/journeys/{fr}.md"]
}}"##
            ),
        )
        .unwrap();
    }
    fs::write(
        repo.path().join("FUNCTIONAL_REQUIREMENTS.md"),
        "- FR-1\n- FR-2\n- FR-3",
    )
    .unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["stats", repo.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("traces: 3"));
}

/// Edge case: a missing `traces/` directory should produce a clear error
/// rather than panic or silently succeed.
#[test]
fn validate_missing_traces_directory_fails() {
    let repo = TempDir::new().unwrap();
    // Intentionally do NOT create a `traces/` subdirectory.

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["validate", repo.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("trace directory not found"));
}

/// Edge case: a trace that references a code_module path which does not exist
/// on disk should fail validation (dangling reference detection).
#[test]
fn validate_dangling_code_module_path_fails() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    fs::write(
        repo.path().join("traces/FR-dangling.json"),
        r##"{
  "fr_id": "FR-dangling",
  "spec_slug": "eco-024-traceability",
  "spec_anchor": "#fr-dangling",
  "docs_pages": [],
  "tests": [],
  "code_modules": ["src/does_not_exist.rs"],
  "journeys": []
}"##,
    )
    .unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["validate", repo.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("dangling path"));
}

/// Edge case: a trace that carries an absolute path reference (starting with `/`)
/// should be rejected as malformed rather than allowed to reach the filesystem.
#[test]
fn validate_absolute_path_in_trace_fails() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    fs::write(
        repo.path().join("traces/FR-abs.json"),
        r##"{
  "fr_id": "FR-abs",
  "spec_slug": "eco-024-traceability",
  "spec_anchor": "#fr-abs",
  "docs_pages": ["/etc/passwd"],
  "tests": [],
  "code_modules": [],
  "journeys": []
}"##,
    )
    .unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["validate", repo.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("malformed path"));
}
