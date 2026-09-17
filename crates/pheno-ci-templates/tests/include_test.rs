//! Integration test for `pheno-ci-templates`.
//!
//! The crate's deliverable is a set of YAML files under
//! `.github/workflows/`. This test walks each of them,
//! asserts they exist, and asserts the minimal structural
//! shape (a `name:` key, an `on.workflow_call:` block).
//! Drift in either is a regression — the whole point of
//! the crate is to be a stable, versioned pin for these
//! files.

use pheno_ci_templates::{all_present, ALL_PATHS};
use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &str) -> String {
    let p = crate_root().join(path);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("failed to read {p:?}: {e}"))
}

#[test]
fn all_workflow_files_exist() {
    assert!(
        all_present(),
        "expected all of {ALL_PATHS:?} to exist under the crate root",
    );
}

#[test]
fn every_workflow_declares_a_workflow_call_trigger() {
    for p in ALL_PATHS {
        let body = read(p);
        assert!(
            body.contains("on:"),
            "{p}: missing top-level `on:` block, body:\n{body}",
        );
        assert!(
            body.contains("workflow_call"),
            "{p}: missing `workflow_call` trigger, body:\n{body}",
        );
    }
}

#[test]
fn every_workflow_has_a_name() {
    for p in ALL_PATHS {
        let body = read(p);
        assert!(
            body.lines().any(|l| l.starts_with("name:")),
            "{p}: missing top-level `name:`, body:\n{body}",
        );
    }
}
