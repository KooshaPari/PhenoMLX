//! Frontend topology audit commands.
//!
//! Traces to: 20260424 stabilization frontend topology gate.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use serde::Serialize;

#[derive(Debug, Args)]
pub struct FrontendArgs {
    #[command(subcommand)]
    pub command: FrontendCommand,
}

#[derive(Debug, Subcommand)]
pub enum FrontendCommand {
    /// Audit frontend directories for manifest or explicit scaffold/archive status.
    Audit(FrontendAuditArgs),
}

#[derive(Debug, Args)]
pub struct FrontendAuditArgs {
    /// Emit JSON instead of a table.
    #[arg(long)]
    pub json: bool,

    /// Return a non-zero exit code when topology gaps are found.
    #[arg(long)]
    pub strict: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FrontendTopologyReport {
    pub entries: Vec<FrontendEntry>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FrontendEntry {
    pub path: String,
    pub status: FrontendStatus,
    pub manifest_present: bool,
    pub status_marker_present: bool,
    pub runnable: bool,
    pub commands: Vec<String>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendStatus {
    Active,
    Scaffold,
    Archive,
    Broken,
    Missing,
}

impl FrontendTopologyReport {
    fn has_gaps(&self) -> bool {
        self.entries.iter().any(|entry| !entry.issues.is_empty())
    }
}

pub async fn run(args: FrontendArgs, repo_root: &Path) -> Result<()> {
    match args.command {
        FrontendCommand::Audit(a) => run_audit(a, repo_root).await,
    }
}

async fn run_audit(args: FrontendAuditArgs, repo_root: &Path) -> Result<()> {
    let report = audit_frontends(repo_root)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }

    if args.strict && report.has_gaps() {
        let gap_count: usize = report.entries.iter().map(|entry| entry.issues.len()).sum();
        bail!("frontend topology audit failed: {gap_count} issue(s)");
    }

    Ok(())
}

fn audit_frontends(repo_root: &Path) -> Result<FrontendTopologyReport> {
    let candidates = [
        FrontendCandidate::new(
            "docs",
            vec!["npm --prefix docs run docs:dev", "npm --prefix docs run docs:build"],
        ),
        FrontendCandidate::new(
            "crates/agileplus-dashboard/web",
            vec!["npm run dev", "npm run test", "npm run build-storybook"],
        ),
    ];

    let mut entries = Vec::new();
    for candidate in candidates {
        entries.push(audit_candidate(repo_root, &candidate)?);
    }

    Ok(FrontendTopologyReport { entries })
}

fn audit_candidate(repo_root: &Path, candidate: &FrontendCandidate) -> Result<FrontendEntry> {
    let path = repo_root.join(&candidate.path);
    let manifest = path.join("package.json");
    let status_marker = path.join("FRONTEND_STATUS.md");
    let manifest_present = manifest.is_file();
    let status_marker_present = status_marker.is_file();
    let marker_status =
        if status_marker_present { Some(read_marker_status(&status_marker)?) } else { None };

    let status = if !path.exists() {
        FrontendStatus::Missing
    } else if manifest_present {
        FrontendStatus::Active
    } else {
        match marker_status {
            Some(FrontendStatus::Scaffold) => FrontendStatus::Scaffold,
            Some(FrontendStatus::Archive) => FrontendStatus::Archive,
            _ => FrontendStatus::Broken,
        }
    };

    let runnable = matches!(status, FrontendStatus::Active);
    let mut issues = Vec::new();

    if matches!(status, FrontendStatus::Missing) {
        issues.push("frontend directory is missing".to_string());
    } else if matches!(status, FrontendStatus::Broken) {
        issues.push(
            "frontend directory lacks package.json and explicit scaffold/archive marker"
                .to_string(),
        );
    } else if matches!(status, FrontendStatus::Scaffold | FrontendStatus::Archive)
        && manifest_present
    {
        issues.push("status marker conflicts with package.json manifest".to_string());
    }

    Ok(FrontendEntry {
        path: candidate.path.to_string_lossy().to_string(),
        status,
        manifest_present,
        status_marker_present,
        runnable,
        commands: if runnable { candidate.commands.clone() } else { Vec::new() },
        issues,
    })
}

fn read_marker_status(path: &Path) -> Result<FrontendStatus> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("reading frontend status marker {}", path.display()))?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(status) = line.strip_prefix("Status:") {
            return parse_status(status.trim())
                .with_context(|| format!("parsing frontend status marker {}", path.display()));
        }
        break;
    }
    bail!(
        "frontend status marker {} must start with `Status: scaffold` or `Status: archive`",
        path.display()
    )
}

fn parse_status(value: &str) -> Result<FrontendStatus> {
    match value {
        "scaffold" => Ok(FrontendStatus::Scaffold),
        "archive" => Ok(FrontendStatus::Archive),
        other => bail!("unsupported frontend status `{other}`"),
    }
}

fn print_report(report: &FrontendTopologyReport) {
    for entry in &report.entries {
        println!("Frontend: {}", entry.path);
        println!("  status: {:?}", entry.status);
        println!("  manifest: {}", entry.manifest_present);
        println!("  status marker: {}", entry.status_marker_present);
        println!("  runnable: {}", entry.runnable);
        for command in &entry.commands {
            println!("  command: {command}");
        }
        for issue in &entry.issues {
            println!("  issue: {issue}");
        }
    }
}

struct FrontendCandidate {
    path: PathBuf,
    commands: Vec<String>,
}

impl FrontendCandidate {
    fn new(path: impl Into<PathBuf>, commands: Vec<&str>) -> Self {
        Self { path: path.into(), commands: commands.into_iter().map(str::to_string).collect() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_accepts_manifest_frontend_and_marked_scaffold() {
        let temp = tempfile::tempdir().unwrap();
        let docs = temp.path().join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("package.json"), "{}\n").unwrap();

        let scaffold = temp.path().join("crates/agileplus-dashboard/web");
        std::fs::create_dir_all(&scaffold).unwrap();
        std::fs::write(scaffold.join("FRONTEND_STATUS.md"), "Status: scaffold\n").unwrap();

        let report = audit_frontends(temp.path()).unwrap();

        assert!(!report.has_gaps());
        assert_eq!(report.entries[0].status, FrontendStatus::Active);
        assert!(report.entries[0].runnable);
        assert_eq!(report.entries[1].status, FrontendStatus::Scaffold);
        assert!(!report.entries[1].runnable);
    }

    #[test]
    fn audit_flags_unmarked_frontend_without_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let docs = temp.path().join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("package.json"), "{}\n").unwrap();
        std::fs::create_dir_all(temp.path().join("crates/agileplus-dashboard/web")).unwrap();

        let report = audit_frontends(temp.path()).unwrap();

        assert!(report.has_gaps());
        assert_eq!(report.entries[1].status, FrontendStatus::Broken);
        assert_eq!(report.entries[1].issues.len(), 1);
    }

    #[test]
    fn marker_status_rejects_unknown_values() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("FRONTEND_STATUS.md");
        std::fs::write(&marker, "Status: active\n").unwrap();

        let err = read_marker_status(&marker).unwrap_err();
        assert!(format!("{err:#}").contains("unsupported frontend status"));
    }
}
