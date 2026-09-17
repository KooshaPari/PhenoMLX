// SPDX-License-Identifier: MIT OR Apache-2.0
use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use tempfile::TempDir;

/// Edge case: a trace file whose payload is *only* whitespace (no JSON tokens
/// at all) must fail validation rather than panic or be silently treated as
/// an empty object. This complements the zero-byte empty-payload test.
#[test]
fn validate_whitespace_only_payload_fails() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    fs::write(repo.path().join("traces/FR-whitespace.json"), "   \n\t  \n").unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["validate", repo.path().to_str().unwrap()])
        .assert()
        .failure();
}

/// Edge case: a trace file containing a JSON value of the wrong shape
/// (a top-level array instead of the expected object) must fail validation
/// with a clear schema error rather than crashing the validator.
#[test]
fn validate_malformed_json_array_payload_fails() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    // Valid JSON syntax (a top-level array) but the wrong shape for a trace.
    fs::write(
        repo.path().join("traces/FR-array.json"),
        r##"["not", "a", "trace", "object"]"##,
    )
    .unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["validate", repo.path().to_str().unwrap()])
        .assert()
        .failure();
}

/// Edge case: many traces each carrying many links should all be counted,
/// and stats output should reflect the aggregate across every category.
/// This stresses the multi-trace + multi-link code path together.
#[test]
fn validate_many_traces_with_many_links_succeeds() {
    let repo = TempDir::new().unwrap();
    fs::create_dir_all(repo.path().join("traces")).unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::create_dir_all(repo.path().join("docs")).unwrap();
    fs::create_dir_all(repo.path().join("docs/operations/journeys")).unwrap();

    // Create the target files that will be referenced.
    for path in [
        "docs/trace.md",
        "docs/architecture.md",
        "src/lib.rs",
        "src/main.rs",
    ] {
        fs::write(repo.path().join(path), "stub").unwrap();
    }

    let trace_count = 4;
    let fr_ids: Vec<String> = (1..=trace_count).map(|i| format!("FR-multi-{i}")).collect();
    for fr in &fr_ids {
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
  "spec_slug": "eco-024-traceability",
  "spec_anchor": "#{fr}",
  "docs_pages": ["docs/trace.md", "docs/architecture.md"],
  "tests": ["tests/edge_cases_extra.rs::validate_many_traces_with_many_links_succeeds"],
  "code_modules": ["src/lib.rs", "src/main.rs"],
  "journeys": ["docs/operations/journeys/{fr}.md"]
}}"##
            ),
        )
        .unwrap();
    }

    // FUNCTIONAL_REQUIREMENTS.md must list every FR or the validator will
    // treat the missing traces as errors. The aggregated link count is
    // (2 docs + 1 test + 2 code + 1 journey) * trace_count = 6 * 4 = 24.
    let reqs = fr_ids
        .iter()
        .map(|fr| format!("- {fr}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(repo.path().join("FUNCTIONAL_REQUIREMENTS.md"), reqs).unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["stats", repo.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains(format!("traces: {trace_count}")))
        .stdout(contains("references: 24"));
}

/// Edge case: a trace whose array fields are all *present* but *empty* should
/// still validate successfully (the schema requires the arrays, not their
/// contents). This guards against an off-by-one regression in `read_trace`
/// where empty arrays might be mistaken for missing fields.
#[test]
fn validate_trace_with_empty_arrays_succeeds() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    fs::write(
        repo.path().join("FUNCTIONAL_REQUIREMENTS.md"),
        "- FR-empty-arrays",
    )
    .unwrap();
    fs::write(
        repo.path().join("traces/FR-empty-arrays.json"),
        r##"{
  "fr_id": "FR-empty-arrays",
  "spec_slug": "eco-024-traceability",
  "spec_anchor": "#fr-empty-arrays",
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
        .success()
        .stdout(contains("validated 1 trace files"));

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["stats", repo.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("traces: 1"))
        .stdout(contains("references: 0"));
}

/// Edge case: two trace files declaring the same `fr_id` should both be
/// counted (the validator's loaders sort and the schema check currently do
/// not de-duplicate). This locks in the existing behavior so a future dedup
/// change is forced to update this test deliberately rather than silently
/// over-count or under-count.
#[test]
fn validate_duplicate_fr_id_counts_both() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    fs::write(repo.path().join("FUNCTIONAL_REQUIREMENTS.md"), "- FR-dup").unwrap();

    let body = r##"{
  "fr_id": "FR-dup",
  "spec_slug": "eco-024-traceability",
  "spec_anchor": "#fr-dup",
  "docs_pages": [],
  "tests": [],
  "code_modules": [],
  "journeys": []
}"##;

    fs::write(repo.path().join("traces/FR-dup.json"), body).unwrap();
    fs::write(repo.path().join("traces/FR-dup-copy.json"), body).unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["stats", repo.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("traces: 2"));
}

/// Edge case: non-JSON / non-trace files (e.g. README.md, .yaml) under
/// `traces/` should be ignored by the validator and the trace count should
/// reflect only the valid `.json` trace files. This guards against accidental
/// over-counting when artifacts are dropped into `traces/`.
#[test]
fn validate_ignores_non_trace_files() {
    let repo = TempDir::new().unwrap();
    fs::create_dir(repo.path().join("traces")).unwrap();
    fs::write(repo.path().join("FUNCTIONAL_REQUIREMENTS.md"), "- FR-only").unwrap();
    fs::write(
        repo.path().join("traces/FR-only.json"),
        r##"{
  "fr_id": "FR-only",
  "spec_slug": "eco-024-traceability",
  "spec_anchor": "#fr-only",
  "docs_pages": [],
  "tests": [],
  "code_modules": [],
  "journeys": []
}"##,
    )
    .unwrap();
    // Non-trace artifacts that should be ignored.
    fs::write(repo.path().join("traces/README.md"), "# notes").unwrap();
    fs::write(repo.path().join("traces/notes.txt"), "notes").unwrap();
    fs::write(repo.path().join("traces/schema.yaml"), "fr_id: FR-only\n").unwrap();

    Command::cargo_bin("agileplus-trace-validator")
        .unwrap()
        .args(["stats", repo.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("traces: 1"));
}
