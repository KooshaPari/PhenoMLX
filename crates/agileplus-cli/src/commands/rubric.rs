// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ap rubric` subcommand — SpecKitty's `score --repo ...` workflow maps here.
//!
//! Thin wrapper around `agileplus_governance::scoring_engine`. The
//! orchestrator loads the rubric catalog, scans the target repo, and
//! renders a v38-formatted scorecard markdown. This CLI is the
//! local-runnable front door: it owns argument parsing, catalog path
//! resolution (walking up to the cargo workspace root), and stdout/file
//! dispatch. See `docs/design/SPECKITTY-MIGRATION.md` for the full
//! mapping from SpecKitty to AgilePlus.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};

use agileplus_governance::scoring_engine::{evaluate, evaluate_with_probes, render_markdown};

/// Default catalog path relative to the cargo workspace root.
const DEFAULT_CATALOG_REL: &str = "crates/agileplus-governance/data/PILLARS-CATALOG.json";

// ── CLI surface ──────────────────────────────────────────────────────────────

/// Top-level `rubric` command group.
#[derive(Debug, Args)]
pub struct RubricArgs {
    #[command(subcommand)]
    pub sub: RubricSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum RubricSubcommand {
    /// Score a repo against the rubric catalog and render the v38 scorecard.
    Score {
        /// Path to the repo root to scan.
        #[arg(long, value_name = "PATH")]
        repo: PathBuf,

        /// Path to a rubric catalog JSON. Defaults to the workspace-bundled
        /// `PILLARS-CATALOG.json` (resolved by walking up to the cargo
        /// workspace root from the current directory).
        #[arg(long, value_name = "PATH")]
        catalog: Option<PathBuf>,

        /// Comma-separated list of cluster ids to score (e.g. `C03,C10,C11`).
        /// When omitted, every cluster in the catalog is scored.
        #[arg(long, value_name = "IDS", value_delimiter = ',')]
        clusters: Option<Vec<String>>,

        /// Write the scorecard to this file instead of stdout.
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,

        /// Probe mode for the v2 content-probe rule registry. `auto` (default)
        /// runs the built-in [`agileplus_governance::scoring_engine::SCORING_PROBES`]
        /// catalog; `none` disables probes (v1 path-presence-only behavior).
        #[arg(long, value_name = "MODE", default_value = "auto")]
        probes: ProbeMode,
    },

    /// Emit a prioritized Markdown fix list from a repo's v38 scorecard.
    FixList(super::fix_list::FixListArgs),
}

/// CLI-facing probe mode. `auto` runs the built-in catalog; `none`
/// disables probes entirely (v1 behavior); `all` is reserved for future
/// use when user-supplied probe catalogs are supported.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ProbeMode {
    Auto,
    None,
    All,
}

// ── Public dispatch ──────────────────────────────────────────────────────────

/// Top-level entry point for the `ap rubric` command group.
///
/// Resolves the catalog path (if the caller didn't supply one), runs the
/// governance orchestrator, renders the markdown, and writes it to stdout
/// or the requested output file. A one-line summary is printed to stdout
/// in addition to the scorecard so callers can capture a parseable
/// "scored N clusters across M pillars (total X/Y, grade Z)" footer.
pub fn run(args: &RubricArgs) -> Result<()> {
    match &args.sub {
        RubricSubcommand::Score {
            repo,
            catalog,
            clusters,
            output,
            probes,
        } => {
            if !repo.exists() {
                bail!("--repo path does not exist: {}", repo.display());
            }
            if !repo.is_dir() {
                bail!("--repo must be a directory: {}", repo.display());
            }

            let catalog_path = match catalog {
                Some(p) => p.clone(),
                None => resolve_default_catalog()?,
            };
            if !catalog_path.exists() {
                bail!(
                    "rubric catalog not found at {} (pass --catalog <path> to override)",
                    catalog_path.display()
                );
            }

            let cluster_filter: Vec<String> = clusters.clone().unwrap_or_default();
            let report = match probes {
                ProbeMode::None => evaluate(repo, &catalog_path, &cluster_filter)
                    .with_context(|| format!("scoring {}", repo.display()))?,
                ProbeMode::Auto | ProbeMode::All => {
                    evaluate_with_probes(repo, &catalog_path, &cluster_filter, None)
                        .with_context(|| format!("scoring {} (probes=enabled)", repo.display()))?
                }
            };
            let markdown = render_markdown(&report);

            match output {
                Some(path) => {
                    std::fs::write(path, &markdown)
                        .with_context(|| format!("writing scorecard to {}", path.display()))?;
                }
                None => {
                    print!("{markdown}");
                }
            }

            // Summary footer — written to stdout regardless of --output so the
            // CLI line stays observable even when the body is redirected.
            let total_points: u32 = report.clusters.iter().map(|c| c.total_points).sum();
            let max_points: u32 = report.clusters.iter().map(|c| c.max_points).sum();
            let pillars: usize = report.clusters.iter().map(|c| c.pillars.len()).sum();
            let pct: u32 = if max_points == 0 {
                0
            } else {
                (total_points * 100) / max_points
            };
            let grade = grade_for_pct(pct);
            println!(
                "scored {} clusters across {} pillars (total {}/{}, grade {})",
                report.clusters.len(),
                pillars,
                total_points,
                max_points,
                grade,
            );

            Ok(())
        }
        RubricSubcommand::FixList(args) => {
            super::fix_list::run(args)
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Walk up the directory tree from `cwd` looking for the first directory
/// that contains `crates/agileplus-governance/data/PILLARS-CATALOG.json`.
/// Falls back to `./crates/agileplus-governance/data/PILLARS-CATALOG.json`
/// (relative to `cwd`) when no ancestor owns the file — this matches the
/// cargo workspace convention.
fn resolve_default_catalog() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("reading current working directory")?;
    resolve_default_catalog_from(&cwd)
}

/// Public alias for use by sibling commands (e.g. `ap cockpit publish`,
/// `ap rubric fix-list`). Resolves the bundled catalog without forcing
/// callers to thread a `cwd` through their own argument surface.
pub(crate) fn resolve_default_catalog_for_siblings() -> Result<PathBuf> {
    resolve_default_catalog()
}

fn resolve_default_catalog_from(cwd: &Path) -> Result<PathBuf> {
    let mut cursor: Option<&Path> = Some(cwd);
    while let Some(dir) = cursor {
        let candidate = dir.join(DEFAULT_CATALOG_REL);
        if candidate.is_file() {
            return Ok(candidate);
        }
        cursor = dir.parent();
    }
    // Fall back to cwd-relative path so callers still get a clear "file not
    // found" error from the existence check above.
    Ok(cwd.join(DEFAULT_CATALOG_REL))
}

fn grade_for_pct(pct: u32) -> &'static str {
    match pct {
        90..=100 => "A",
        75..=89 => "B",
        60..=74 => "C",
        40..=59 => "D",
        _ => "F",
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grade_for_pct_handles_boundaries() {
        assert_eq!(grade_for_pct(100), "A");
        assert_eq!(grade_for_pct(90), "A");
        assert_eq!(grade_for_pct(89), "B");
        assert_eq!(grade_for_pct(75), "B");
        assert_eq!(grade_for_pct(60), "C");
        assert_eq!(grade_for_pct(40), "D");
        assert_eq!(grade_for_pct(0), "F");
    }

    #[test]
    fn resolve_default_catalog_walks_up_to_workspace_root() {
        // If the worktree layout is intact, this crate's source tree is a
        // descendant of the workspace root that owns PILLARS-CATALOG.json.
        let resolved = resolve_default_catalog().expect("catalog should resolve");
        assert!(
            resolved.is_file(),
            "expected default catalog to exist at {}",
            resolved.display()
        );
        assert!(
            resolved.ends_with("PILLARS-CATALOG.json"),
            "unexpected default catalog path: {}",
            resolved.display()
        );
    }
}