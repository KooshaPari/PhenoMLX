//! Spec truth audit commands.
//!
//! Traces to: FR-014, FR-042 / 20260424 stabilization.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use serde::Serialize;

use agileplus_sqlite::SqliteStorageAdapter;

#[derive(Debug, Args)]
pub struct SpecsArgs {
    #[command(subcommand)]
    pub command: SpecsCommand,
}

#[derive(Debug, Subcommand)]
pub enum SpecsCommand {
    /// Audit canonical spec root parity against the AgilePlus database.
    Audit(SpecAuditArgs),
}

#[derive(Debug, Args)]
pub struct SpecAuditArgs {
    /// Canonical spec directory.
    #[arg(long, default_value = ".agileplus/specs")]
    pub spec_root: PathBuf,

    /// Legacy/mirror roots to report when canonical specs are missing.
    #[arg(long = "legacy-root", default_values = ["kitty-specs", "docs/specs"])]
    pub legacy_roots: Vec<PathBuf>,

    /// Emit JSON instead of a table.
    #[arg(long)]
    pub json: bool,

    /// Return a non-zero exit code when parity gaps are found.
    #[arg(long)]
    pub strict: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SpecAuditReport {
    pub spec_root: String,
    pub feature_count: usize,
    pub canonical_spec_count: usize,
    pub missing_canonical_specs: Vec<String>,
    pub orphaned_canonical_specs: Vec<String>,
    pub legacy_matches: Vec<LegacySpecMatch>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LegacySpecMatch {
    pub slug: String,
    pub root: String,
}

impl SpecAuditReport {
    fn has_gaps(&self) -> bool {
        !self.missing_canonical_specs.is_empty() || !self.orphaned_canonical_specs.is_empty()
    }
}

pub async fn run(args: SpecsArgs, storage: &SqliteStorageAdapter, repo_root: &Path) -> Result<()> {
    match args.command {
        SpecsCommand::Audit(a) => run_audit(a, storage, repo_root).await,
    }
}

async fn run_audit(
    args: SpecAuditArgs,
    storage: &SqliteStorageAdapter,
    repo_root: &Path,
) -> Result<()> {
    let slugs = storage.list_feature_slugs().context("listing feature slugs")?;
    let report = audit_specs(
        &slugs,
        &resolve_path(repo_root, &args.spec_root),
        &args.legacy_roots.iter().map(|root| resolve_path(repo_root, root)).collect::<Vec<_>>(),
    )?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }

    if args.strict && report.has_gaps() {
        bail!(
            "spec audit failed: {} missing canonical specs, {} orphaned canonical specs",
            report.missing_canonical_specs.len(),
            report.orphaned_canonical_specs.len()
        );
    }

    Ok(())
}

fn audit_specs(
    feature_slugs: &[String],
    spec_root: &Path,
    legacy_roots: &[PathBuf],
) -> Result<SpecAuditReport> {
    let feature_slugs = feature_slugs.iter().cloned().collect::<BTreeSet<_>>();
    let canonical_slugs = spec_slugs(spec_root)?;

    let missing_canonical_specs =
        feature_slugs.difference(&canonical_slugs).cloned().collect::<Vec<_>>();
    let orphaned_canonical_specs =
        canonical_slugs.difference(&feature_slugs).cloned().collect::<Vec<_>>();

    let mut legacy_matches = Vec::new();
    for root in legacy_roots {
        let slugs = spec_slugs(root)?;
        for slug in &missing_canonical_specs {
            if slugs.contains(slug) {
                legacy_matches
                    .push(LegacySpecMatch { slug: slug.clone(), root: display_path(root) });
            }
        }
    }

    Ok(SpecAuditReport {
        spec_root: display_path(spec_root),
        feature_count: feature_slugs.len(),
        canonical_spec_count: canonical_slugs.len(),
        missing_canonical_specs,
        orphaned_canonical_specs,
        legacy_matches,
    })
}

fn spec_slugs(root: &Path) -> Result<BTreeSet<String>> {
    if !root.exists() {
        return Ok(BTreeSet::new());
    }

    let mut slugs = BTreeSet::new();
    for entry in std::fs::read_dir(root).with_context(|| format!("reading {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() || !path.join("spec.md").is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            slugs.insert(name.to_string());
        }
    }
    Ok(slugs)
}

fn print_report(report: &SpecAuditReport) {
    println!("Spec root: {}", report.spec_root);
    println!("Features: {}", report.feature_count);
    println!("Canonical specs: {}", report.canonical_spec_count);
    println!("Missing canonical specs: {}", report.missing_canonical_specs.len());
    for slug in &report.missing_canonical_specs {
        println!("  missing: {slug}");
    }
    println!("Orphaned canonical specs: {}", report.orphaned_canonical_specs.len());
    for slug in &report.orphaned_canonical_specs {
        println!("  orphaned: {slug}");
    }
    if !report.legacy_matches.is_empty() {
        println!("Legacy matches:");
        for entry in &report.legacy_matches {
            println!("  {} -> {}", entry.slug, entry.root);
        }
    }
}

fn resolve_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { repo_root.join(path) }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_reports_missing_orphaned_and_legacy_matches() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().join(".agileplus/specs");
        let legacy = temp.path().join("kitty-specs");
        std::fs::create_dir_all(canonical.join("present")).unwrap();
        std::fs::write(canonical.join("present/spec.md"), "# Present\n").unwrap();
        std::fs::create_dir_all(canonical.join("orphan")).unwrap();
        std::fs::write(canonical.join("orphan/spec.md"), "# Orphan\n").unwrap();
        std::fs::create_dir_all(legacy.join("legacy-only")).unwrap();
        std::fs::write(legacy.join("legacy-only/spec.md"), "# Legacy\n").unwrap();

        let report = audit_specs(
            &["present".into(), "legacy-only".into(), "missing".into()],
            &canonical,
            &[legacy],
        )
        .unwrap();

        assert_eq!(report.feature_count, 3);
        assert_eq!(report.canonical_spec_count, 2);
        assert_eq!(
            report.missing_canonical_specs,
            vec!["legacy-only".to_string(), "missing".to_string()]
        );
        assert_eq!(report.orphaned_canonical_specs, vec!["orphan".to_string()]);
        assert_eq!(report.legacy_matches.len(), 1);
        assert_eq!(report.legacy_matches[0].slug, "legacy-only");
    }

    #[test]
    fn missing_roots_are_empty_not_errors() {
        let temp = tempfile::tempdir().unwrap();
        let report = audit_specs(&["missing".into()], &temp.path().join("none"), &[]).unwrap();
        assert_eq!(report.missing_canonical_specs, vec!["missing".to_string()]);
        assert!(report.orphaned_canonical_specs.is_empty());
    }
}
