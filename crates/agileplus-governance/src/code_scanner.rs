//! Code scanner: walk a repository and extract structural evidence facts that
//! the SpecKitty scoring engine consumes (design §4.1).
//!
//! Dependency-free (std::fs only) — the scanner just needs file-presence, counts,
//! and shallow content probes, not a full ignore-aware walker. Facts are emitted as
//! [`EvidenceItem`]s keyed by an `artifact_id` the scoring rules match on.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{GovernanceError, Result};

/// A single structural fact about a repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceItem {
    /// Stable key scoring rules match on, e.g. `file:AGENTS.md`, `count:test_files`.
    pub artifact_id: String,
    /// Fact kind, e.g. `file_presence`, `count`, `content_probe`.
    pub kind: String,
    /// Repo-relative path this fact refers to (empty for aggregate counts).
    #[serde(default)]
    pub path: String,
    /// Free-form metadata (bool/number as strings; keeps the type flat + serde-simple).
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl EvidenceItem {
    fn presence(artifact_id: &str, path: &str, present: bool) -> Self {
        let mut metadata = BTreeMap::new();
        metadata.insert("present".into(), present.to_string());
        EvidenceItem {
            artifact_id: artifact_id.into(),
            kind: "file_presence".into(),
            path: path.into(),
            metadata,
        }
    }

    fn count(artifact_id: &str, n: usize) -> Self {
        let mut metadata = BTreeMap::new();
        metadata.insert("count".into(), n.to_string());
        EvidenceItem {
            artifact_id: artifact_id.into(),
            kind: "count".into(),
            path: String::new(),
            metadata,
        }
    }

    /// Read `present` as bool (defaults false).
    pub fn present(&self) -> bool {
        self.metadata.get("present").map(|v| v == "true").unwrap_or(false)
    }

    /// Read `count` as usize (defaults 0).
    pub fn count_value(&self) -> usize {
        self.metadata.get("count").and_then(|v| v.parse().ok()).unwrap_or(0)
    }
}

/// Directory names skipped during the walk (build output, VCS, deps).
const SKIP_DIRS: &[&str] = &[
    ".git", "target", "node_modules", ".claude", "dist", "build", ".venv", "vendor",
];

/// Repo-root files whose presence is a scored signal.
const PRESENCE_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "llms.txt",
    "README.md",
    "CHANGELOG.md",
    "deny.toml",
    "rust-toolchain.toml",
    ".github/PULL_REQUEST_TEMPLATE.md",
    "docs/functional_requirements.md",
    "docs/friction-log.md",
    "Dockerfile",
    "Containerfile",
];

/// Structural evidence scanned from one repository.
#[derive(Debug, Clone, Default)]
pub struct RepoScan {
    /// All emitted facts.
    pub items: Vec<EvidenceItem>,
}

impl RepoScan {
    /// Look up a fact by artifact_id.
    pub fn get(&self, artifact_id: &str) -> Option<&EvidenceItem> {
        self.items.iter().find(|i| i.artifact_id == artifact_id)
    }

    /// True if a scored presence-file exists.
    pub fn has(&self, repo_relative: &str) -> bool {
        self.get(&format!("file:{repo_relative}")).map(|i| i.present()).unwrap_or(false)
    }
}

/// Walk `repo_root` and collect structural evidence.
pub fn scan_repo(repo_root: impl AsRef<Path>) -> Result<RepoScan> {
    let root = repo_root.as_ref();
    if !root.is_dir() {
        return Err(GovernanceError::Rubric(format!(
            "scan target is not a directory: {}",
            root.display()
        )));
    }

    let mut items = Vec::new();

    // Root-file presence signals.
    for rel in PRESENCE_FILES {
        let present = root.join(rel).exists();
        items.push(EvidenceItem::presence(&format!("file:{rel}"), rel, present));
    }

    // Rust test-file count (files containing a #[test]/#[tokio::test] attribute).
    let rs_files = collect_files(root, "rs");
    let test_files = rs_files
        .iter()
        .filter(|p| file_contains_any(p, &["#[test]", "#[tokio::test]"]))
        .count();
    items.push(EvidenceItem::count("count:rs_files", rs_files.len()));
    items.push(EvidenceItem::count("count:test_files", test_files));

    // Workspace crate count (Cargo.toml files below root).
    let crate_manifests = collect_named(root, "Cargo.toml").len();
    items.push(EvidenceItem::count("count:cargo_manifests", crate_manifests));

    // CI workflow presence.
    let ci = root.join(".github/workflows").is_dir()
        && collect_files(&root.join(".github/workflows"), "yml").len() > 0;
    items.push(EvidenceItem::presence("dir:.github/workflows", ".github/workflows", ci));

    Ok(RepoScan { items })
}

/// Recursively collect files with the given extension, skipping SKIP_DIRS.
fn collect_files(root: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, &mut |p| {
        if p.extension().and_then(|e| e.to_str()) == Some(ext) {
            out.push(p.to_path_buf());
        }
    });
    out
}

/// Recursively collect files with an exact name.
fn collect_named(root: &Path, name: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, &mut |p| {
        if p.file_name().and_then(|n| n.to_str()) == Some(name) {
            out.push(p.to_path_buf());
        }
    });
    out
}

/// Depth-first walk applying `f` to each file, skipping SKIP_DIRS.
fn walk(dir: &Path, f: &mut dyn FnMut(&Path)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if SKIP_DIRS.contains(&name) {
                continue;
            }
            walk(&path, f);
        } else {
            f(&path);
        }
    }
}

/// True if the file contains any of the given needles.
fn file_contains_any(path: &Path, needles: &[&str]) -> bool {
    match std::fs::read_to_string(path) {
        Ok(s) => needles.iter().any(|n| s.contains(n)),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_repo() -> PathBuf {
        // Unique per-test dir under the OS temp root (no external tempfile dep).
        let base = std::env::temp_dir().join(format!("speckitty-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join(".github/workflows")).unwrap();
        fs::create_dir_all(base.join("src")).unwrap();
        fs::create_dir_all(base.join("target/debug")).unwrap(); // must be skipped
        base
    }

    #[test]
    fn scans_presence_and_counts() {
        let repo = tmp_repo();
        fs::write(repo.join("AGENTS.md"), "# agents").unwrap();
        fs::write(repo.join("Cargo.toml"), "[package]").unwrap();
        fs::write(repo.join(".github/workflows/ci.yml"), "on: push").unwrap();
        fs::write(repo.join("src/lib.rs"), "#[test] fn t() {}").unwrap();
        fs::write(repo.join("target/debug/junk.rs"), "#[test] fn skip() {}").unwrap();

        let scan = scan_repo(&repo).unwrap();
        assert!(scan.has("AGENTS.md"));
        assert!(!scan.has("CHANGELOG.md"));
        assert!(scan.get("dir:.github/workflows").unwrap().present());
        // target/ is skipped, so only src/lib.rs counts as a test file.
        assert_eq!(scan.get("count:test_files").unwrap().count_value(), 1);
        assert_eq!(scan.get("count:cargo_manifests").unwrap().count_value(), 1);

        fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn errors_on_missing_dir() {
        let err = scan_repo("/nonexistent/speckitty/path").unwrap_err();
        assert!(matches!(err, GovernanceError::Rubric(_)));
    }
}
