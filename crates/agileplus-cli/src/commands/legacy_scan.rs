//! Legacy tooling anti-pattern scan command.
//!
//! Scans repository for banned tooling patterns per Phenotype Technology
//! Adoption Philosophy (CLAUDE.md). Wraps the shared Python scanner.
//!
//! Traceability: FR-LEGACY-SCAN-001

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use clap::Args;

/// Arguments for the legacy-scan command.
#[derive(Debug, Args)]
pub struct LegacyScanArgs {
    /// Repository root to scan (defaults to current directory)
    #[arg(long, default_value = ".")]
    pub repo_root: PathBuf,

    /// Minimum severity to fail on
    #[arg(long, default_value = "high")]
    pub severity: String,

    /// Report only, do not fail
    #[arg(long)]
    pub report_only: bool,

    /// Output JSON report path
    #[arg(long)]
    pub output: Option<PathBuf>,
}

/// Run the legacy tooling scanner.
pub async fn run(args: LegacyScanArgs) -> Result<()> {
    // Find the phenotype/repos root relative to AgilePlus
    let agileplus_root = find_agileplus_root()?;
    let repos_root = agileplus_root
        .parent()
        .context("AgilePlus has no parent directory")?;

    let scanner_path = repos_root
        .join("tooling")
        .join("legacy-enforcement")
        .join("scanner")
        .join("legacy_tooling_scanner.py");

    let policy_path = repos_root
        .join("tooling")
        .join("legacy-enforcement")
        .join("policy")
        .join("rules.yaml");

    if !scanner_path.exists() {
        anyhow::bail!(
            "Scanner not found at {}. Is phenotype/repos checked out?",
            scanner_path.display()
        );
    }

    println!("Scanning: {}", args.repo_root.display());
    println!("Policy: {}", policy_path.display());
    println!("Threshold: {}", args.severity);
    println!("{}", "-".repeat(70));

    let mut cmd = Command::new("python3");
    cmd.arg(&scanner_path)
        .arg("--repo-root")
        .arg(&args.repo_root)
        .arg("--policy")
        .arg(&policy_path)
        .arg("--fail-on-severity")
        .arg(&args.severity);

    if args.report_only {
        cmd.arg("--report-only");
    }

    if let Some(ref output) = args.output {
        cmd.arg("--output-json").arg(output);
    }

    let status = cmd
        .status()
        .context("failed to execute legacy tooling scanner")?;

    match status.code() {
        Some(0) => {
            println!("\n[ok] No blocking issues found");
            Ok(())
        }
        Some(2) => {
            if args.report_only {
                println!("\n[!] Violations detected (report-only mode)");
                Ok(())
            } else {
                anyhow::bail!("Blocking violations detected - fix required");
            }
        }
        Some(code) => {
            anyhow::bail!("Scanner exited with code {}", code);
        }
        None => {
            anyhow::bail!("Scanner was terminated");
        }
    }
}

/// Find the AgilePlus repository root by walking up from current directory.
fn find_agileplus_root() -> Result<PathBuf> {
    let mut current = std::env::current_dir()?;

    loop {
        // Check if this looks like AgilePlus (has crates/agileplus-cli)
        if current.join("crates").join("agileplus-cli").exists() {
            return Ok(current);
        }

        // Check for .git and AGENTS.md as fallback
        if current.join("AGENTS.md").exists() && current.join(".git").exists() {
            let name = current.file_name().and_then(|n| n.to_str());
            if name == Some("AgilePlus") {
                return Ok(current);
            }
        }

        if !current.pop() {
            break;
        }
    }

    // Fallback: assume we're in repos/AgilePlus or repos/something
    let cwd = std::env::current_dir()?;
    if cwd.join("..").join("AgilePlus").exists() {
        return Ok(cwd.join("..").join("AgilePlus").canonicalize()?);
    }

    anyhow::bail!(
        "Could not find AgilePlus root. Run from within the AgilePlus repository."
    )
}
