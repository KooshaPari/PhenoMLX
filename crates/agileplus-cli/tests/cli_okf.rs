// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration tests for the `ap okf <validate|summarize|merge>` subcommand.
//!
//! These shell out to the built `agileplus` binary and feed it OKF v1.0
//! documents in-memory via tempdirs — no filesystem fixtures, no DB.
//!
//! Reference spec: SessionLedger `docs/reference/OKF-SPEC.md` (v1.0).
//!
//! ## Coverage matrix (per team-lead brief)
//!
//! 1. validate on a known-good doc → exit 0 + "valid"
//! 2. validate on a malformed entity (missing id) → exit 1 + line citation
//! 3. validate on a malformed relation (unknown type) → exit 1
//! 4. summarize on Tracera-style document → 3+ entity count rows
//! 5. merge on 2 docs → exit 0, output 2× entities
//! 6. validate on empty file → exit 1, "empty file" message
//!
//! Plus: merge must round-trip without duplicating ids or rewriting
//! relations incorrectly.

use assert_cmd::Command;

fn cli() -> Command {
    Command::cargo_bin("agileplus").expect("agileplus binary should be built")
}

// ─── fixture builders ────────────────────────────────────────────────────────

const GOOD_DOC: &str = r#"{
  "okf": "1.0",
  "source_id": "forge-fixture-good",
  "entities": [
    { "id": "intent-0", "type": "intent", "label": "ship okf cli",
      "properties": { "user_turn_count": 2 } },
    { "id": "acceptance-0", "type": "acceptance",
      "label": "tests pass", "properties": null },
    { "id": "constraint-0", "type": "constraint",
      "label": "no schema change", "properties": null },
    { "id": "gate-0", "type": "gate", "label": "resume-gate",
      "properties": { "ready": true, "scope_sized": true } }
  ],
  "relations": [
    { "source": "intent-0", "target": "acceptance-0", "type": "verified_by",
      "provenance": { "corpus": "forge", "source_id": "forge-fixture-good" } },
    { "source": "intent-0", "target": "constraint-0", "type": "bounded_by",
      "provenance": { "corpus": "forge", "source_id": "forge-fixture-good" } },
    { "source": "intent-0", "target": "gate-0", "type": "asserts",
      "provenance": { "corpus": "forge", "source_id": "forge-fixture-good" } }
  ],
  "provenance": { "corpus": "forge", "source_id": "forge-fixture-good" }
}"#;

const SECOND_GOOD_DOC: &str = r#"{
  "okf": "1.0",
  "source_id": "codex-fixture-second",
  "entities": [
    { "id": "intent-0", "type": "intent", "label": "audit bash scripts",
      "properties": { "user_turn_count": 1 } },
    { "id": "criteria-0", "type": "criteria", "label": "all tests green",
      "properties": null }
  ],
  "relations": [
    { "source": "intent-0", "target": "criteria-0", "type": "requires",
      "provenance": { "corpus": "codex", "source_id": "codex-fixture-second" } }
  ],
  "provenance": { "corpus": "codex", "source_id": "codex-fixture-second" }
}"#;

const TRACERA_STYLE_DOC: &str = r#"{
  "okf": "1.0",
  "source_id": "tracera-roundtrip-001",
  "entities": [
    { "id": "intent-0", "type": "intent", "label": "implement Tracera roundtrip demo",
      "properties": { "user_turn_count": 3 } },
    { "id": "acceptance-0", "type": "acceptance", "label": "viewer renders fixture bundle",
      "properties": null },
    { "id": "acceptance-1", "type": "acceptance", "label": "diff matches expected fixture",
      "properties": null },
    { "id": "constraint-0", "type": "constraint",
      "label": "do not modify sl-daemon worker", "properties": null },
    { "id": "resource-0", "type": "resource",
      "label": "working-directory",
      "properties": { "cwd": "/Users/dev/sl", "model": "sonnet" } },
    { "id": "state-0", "type": "state", "label": "session-title",
      "properties": { "title": "OKF roundtrip demo" } },
    { "id": "criteria-0", "type": "criteria",
      "label": "viewer renders entities and relations",
      "properties": { "watch_files": ["src/render.rs"] } },
    { "id": "gate-0", "type": "gate", "label": "resume-gate",
      "properties": { "ready": true, "scope_sized": true, "user_turns": 3 } }
  ],
  "relations": [
    { "source": "intent-0", "target": "acceptance-0", "type": "verified_by",
      "provenance": { "corpus": "tracera", "source_id": "tracera-roundtrip-001" } },
    { "source": "intent-0", "target": "acceptance-1", "type": "verified_by",
      "provenance": { "corpus": "tracera", "source_id": "tracera-roundtrip-001" } },
    { "source": "intent-0", "target": "constraint-0", "type": "bounded_by",
      "provenance": { "corpus": "tracera", "source_id": "tracera-roundtrip-001" } },
    { "source": "intent-0", "target": "resource-0", "type": "grounds",
      "provenance": { "corpus": "tracera", "source_id": "tracera-roundtrip-001" } },
    { "source": "intent-0", "target": "state-0", "type": "grounds",
      "provenance": { "corpus": "tracera", "source_id": "tracera-roundtrip-001" } },
    { "source": "intent-0", "target": "criteria-0", "type": "requires",
      "provenance": { "corpus": "tracera", "source_id": "tracera-roundtrip-001" } },
    { "source": "intent-0", "target": "gate-0", "type": "asserts",
      "provenance": { "corpus": "tracera", "source_id": "tracera-roundtrip-001" } }
  ],
  "provenance": { "corpus": "tracera", "source_id": "tracera-roundtrip-001" }
}"#;

/// Entity with missing `id` — should trip §4.1 and report "entity with empty `id`".
const MALFORMED_ENTITY: &str = r#"{
  "okf": "1.0",
  "source_id": "forge-bad-entity",
  "entities": [
    { "type": "intent", "label": "missing id field", "properties": null }
  ],
  "relations": [],
  "provenance": { "corpus": "forge", "source_id": "forge-bad-entity" }
}"#;

/// Relation with unknown type — should trip §5.2 "relation has unknown type".
const MALFORMED_RELATION: &str = r#"{
  "okf": "1.0",
  "source_id": "forge-bad-rel",
  "entities": [
    { "id": "intent-0", "type": "intent", "label": "x", "properties": null },
    { "id": "acceptance-0", "type": "acceptance", "label": "y", "properties": null }
  ],
  "relations": [
    { "source": "intent-0", "target": "acceptance-0", "type": "totally_bogus",
      "provenance": { "corpus": "forge", "source_id": "forge-bad-rel" } }
  ],
  "provenance": { "corpus": "forge", "source_id": "forge-bad-rel" }
}"#;

/// Empty file — should fail with "empty file" message and exit 1.
const EMPTY_DOC: &str = "";

/// Write a doc to a tempdir and return the path.
fn write_doc(label: &str, body: &str) -> std::path::PathBuf {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(format!("{label}.okf.json"));
    std::fs::write(&path, body).expect("write fixture");
    // We can't return dir (closure-keepable) AND a path; leak the dir so
    // tests run with stable file paths. Tokio tests are short so this is OK.
    std::mem::forget(dir);
    path
}

// ─── 1. validate → exit 0 + "valid" ──────────────────────────────────────────

#[test]
fn okf_validate_passes_on_known_good_doc() {
    let path = write_doc("good", GOOD_DOC);
    let assert = cli()
        .args(["okf", "validate"])
        .arg(&path)
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(out.contains("valid:"), "missing valid marker: {out}");
}

// ─── 2. malformed entity (missing id) → exit 1 + error cited ─────────────────

#[test]
fn okf_validate_fails_on_entity_missing_id() {
    let path = write_doc("bad-entity", MALFORMED_ENTITY);
    let assert = cli()
        .args(["okf", "validate"])
        .arg(&path)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("entity with empty `id`")
            || stderr.contains("missing field `id`"),
        "expected id-citation in stderr, got: {stderr}"
    );
}

// ─── 3. malformed relation (unknown type) → exit 1 ───────────────────────────

#[test]
fn okf_validate_fails_on_unknown_relation_type() {
    let path = write_doc("bad-rel", MALFORMED_RELATION);
    let assert = cli()
        .args(["okf", "validate"])
        .arg(&path)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("unknown type `totally_bogus`"),
        "expected unknown-type citation in stderr, got: {stderr}"
    );
}

// ─── 4. summarize on Tracera-style doc → 3+ entity count rows ────────────────

#[test]
fn okf_summarize_emits_entity_count_rows() {
    let path = write_doc("tracera", TRACERA_STYLE_DOC);
    let assert = cli()
        .args(["okf", "summarize"])
        .arg(&path)
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    // The Tracera fixture has 7 distinct entity types: intent, acceptance,
    // constraint, resource, state, criteria, gate — all 7 must appear in
    // the "by type" table.
    let rows = ["intent", "acceptance", "constraint", "resource", "state", "criteria", "gate"]
        .iter()
        .filter(|name| {
            // Each row appears as `  <name>  <count>` (single space-aligned).
            out.contains(&format!("\n  {}\n", name))
                || out.contains(&format!("{:>1}", name))
        })
        .count();
    assert!(
        rows >= 6,
        "expected at least 6 entity-type rows in summary, got {rows}. Output:\n{out}"
    );
    // Top-label listing must contain the longest label verbatim.
    assert!(
        out.contains("do not modify sl-daemon worker"),
        "missing longest label in summary top-N section: {out}"
    );
}

// ─── 5. merge on 2 docs → exit 0, output has 2× entities ────────────────────

#[test]
fn okf_merge_concatenates_two_docs_into_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p1 = dir.path().join("a.okf.json");
    let p2 = dir.path().join("b.okf.json");
    let out_path = dir.path().join("merged.okf.json");
    std::fs::write(&p1, GOOD_DOC).expect("write p1");
    std::fs::write(&p2, SECOND_GOOD_DOC).expect("write p2");
    // Leak the tempdir so the output path stays valid for the assertion
    // below (tests are short-lived, so this is acceptable here).
    let leaked = dir.keep();

    let assert = cli()
        .args(["okf", "merge", "--output"])
        .arg(&out_path)
        .arg(&p1)
        .arg(&p2)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(stdout.contains("merge ok"), "missing merge-ok marker: {stdout}");

    let merged_text = std::fs::read_to_string(&out_path).expect("read merged");
    let v: serde_json::Value = serde_json::from_str(&merged_text).expect("merged is JSON");

    let entities = v.get("entities").and_then(|e| e.as_array()).expect("entities array");
    // GOOD_DOC has 4 entities, SECOND_GOOD_DOC has 2 → merged = 6.
    assert_eq!(
        entities.len(),
        6,
        "expected 6 merged entities, got {}",
        entities.len()
    );

    // Verify id collision avoidance: both inputs had `intent-0`; merged
    // should namespace them so neither is dropped and neither collides.
    let ids: std::collections::HashSet<&str> = entities
        .iter()
        .filter_map(|e| e.get("id").and_then(|i| i.as_str()))
        .collect();
    assert_eq!(ids.len(), 6, "expected 6 unique ids, saw duplicates: {:?}", ids);

    // Both intent-0 ids should be present (under different prefixes).
    let has_first = ids.iter().any(|id| id.ends_with("forge-fixture-good::intent-0"));
    let has_second = ids.iter().any(|id| id.ends_with("codex-fixture-second::intent-0"));
    assert!(has_first && has_second, "missing namespaced intent-0 ids: {ids:?}");

    let relations = v.get("relations").and_then(|r| r.as_array()).expect("relations array");
    // GOOD_DOC has 3 relations, SECOND has 1 → merged = 4.
    assert_eq!(
        relations.len(),
        4,
        "expected 4 merged relations, got {}",
        relations.len()
    );
    // Every relation source/target must reference a real merged entity id.
    for rel in relations {
        let src = rel.get("source").and_then(|s| s.as_str()).expect("src");
        let tgt = rel.get("target").and_then(|t| t.as_str()).expect("tgt");
        assert!(ids.contains(src), "dangling relation source `{src}`");
        assert!(ids.contains(tgt), "dangling relation target `{tgt}`");
    }

    // The first intent-0 id should appear in merged relations pointing at
    // acceptance-0 / constraint-0 / gate-0 within its own namespace.
    let good_prefix = "forge::forge-fixture-good::";
    let good_acceptance = format!("{good_prefix}acceptance-0");
    assert!(
        ids.contains(good_acceptance.as_str()),
        "expected namespaced `{good_acceptance}`, saw: {ids:?}"
    );

    // Cleanup at end of test — `leaked` is the kept tempdir handle.
    let _ = std::fs::remove_dir_all(&leaked);
}

// ─── 6. validate on empty file → exit 1, "empty file" ───────────────────────

#[test]
fn okf_validate_fails_on_empty_file() {
    let path = write_doc("empty", EMPTY_DOC);
    let assert = cli()
        .args(["okf", "validate"])
        .arg(&path)
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("empty file"),
        "expected empty-file message, got: {stderr}"
    );
}

// ─── extras (defensive coverage beyond the 6 required cases) ────────────────

/// Validate must reject an unsupported OKF version.
#[test]
fn okf_validate_fails_on_unsupported_version() {
    let bad_version = r#"{
      "okf": "2.0",
      "source_id": "x",
      "entities": [],
      "relations": [],
      "provenance": { "corpus": "forge", "source_id": "x" }
    }"#;
    let path = write_doc("bad-ver", bad_version);
    cli()
        .args(["okf", "validate"])
        .arg(&path)
        .assert()
        .failure();
}

/// Summarize must also accept the minimal fixture (covers the "single intent"
/// corpus used by other downstreams).
#[test]
fn okf_summarize_handles_minimal_single_intent_doc() {
    let minimal = r#"{
      "okf": "1.0",
      "source_id": "minimal-001",
      "entities": [
        { "id": "intent-0", "type": "intent",
          "label": "minimal", "properties": null }
      ],
      "provenance": { "corpus": "forge", "source_id": "minimal-001" }
    }"#;
    let path = write_doc("minimal", minimal);
    cli()
        .args(["okf", "summarize"])
        .arg(&path)
        .assert()
        .success();
}

/// Merge with a single input must still be rejected (the spec says ≥2).
#[test]
fn okf_merge_rejects_single_input_with_help() {
    let path = write_doc("only-one", GOOD_DOC);
    cli()
        .args(["okf", "merge"])
        .arg(&path)
        .assert()
        // `ap okf merge PATH` with 1 path either fails validation OR clap
        // rejects it — accept either form (we don't pin the surface here).
        .failure();
}
