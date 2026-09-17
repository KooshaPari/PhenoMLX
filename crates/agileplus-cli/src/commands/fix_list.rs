// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ap rubric fix-list` — emit a prioritized Markdown fix list from a
//! repo's v38 scorecard.
//!
//! Spec: `ap rubric fix-list --repo <path>` reads every cluster in the
//! bundled rubric catalog, ranks all `score=0` and `score=1` gaps by
//! (ascending-score, then cluster-id ascending), and emits the top N
//! gaps as a Markdown table. Defaults to top 10; configurable with
//! `--limit N`. Output is written to stdout or to a file via
//! `--output <path>`.
//!
//! One-line elevator: a cockpit-friendly, machine-readable diff
//! artifact that pairs with `ap rubric score` so an operator (or a
//! fleet dashboard) can see *what to fix next*, not just *what is
//! currently scored*.
//!
//! Why a separate subcommand rather than a flag on `score`: the
//! scorecard is dense and designed for diff-ability; the fix-list is
//! a flat, ranked view optimized for *action*. Keeping them separate
//! means render pipelines don't need to parse a table inside a
//! markdown doc.

use std::fmt::Write as _;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Args;
use comfy_table::{Cell, Row, Table};

use agileplus_governance::scoring_engine::{evaluate, ClusterScore, ScoreReport};

use crate::commands::rubric::resolve_default_catalog_for_siblings;

/// Top-level CLI args for the `rubric fix-list` subcommand.
#[derive(Debug, Args)]
pub struct FixListArgs {
    /// Path to the repo root to scan.
    #[arg(long, value_name = "PATH")]
    pub repo: PathBuf,

    /// Path to a rubric catalog JSON. Defaults to the workspace-bundled
    /// `PILLARS-CATALOG.json` (same resolution as `ap rubric score`).
    #[arg(long, value_name = "PATH")]
    pub catalog: Option<PathBuf>,

    /// Comma-separated list of cluster ids to include (e.g. `C03,C10,C11`).
    /// When omitted, every cluster in the catalog is scored.
    #[arg(long, value_name = "IDS", value_delimiter = ',')]
    pub clusters: Option<Vec<String>>,

    /// Write the fix list to this file instead of stdout.
    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Maximum number of rows to emit. Defaults to 10 (the spec ceiling).
    #[arg(long, value_name = "N", default_value_t = 10)]
    pub limit: usize,
}

/// Public entry point — mirrors the `pub fn run` shape used by sibling
/// subcommands (`rubric`, `cockpit`).
pub fn run(args: &FixListArgs) -> Result<()> {
    if !args.repo.exists() {
        bail!("--repo path does not exist: {}", args.repo.display());
    }
    if !args.repo.is_dir() {
        bail!("--repo must be a directory: {}", args.repo.display());
    }

    let catalog_path = match &args.catalog {
        Some(p) => p.clone(),
        None => resolve_default_catalog_for_siblings()?,
    };
    if !catalog_path.exists() {
        bail!(
            "rubric catalog not found at {} (pass --catalog <path> to override)",
            catalog_path.display()
        );
    }

    let cluster_filter: Vec<String> = args.clusters.clone().unwrap_or_default();
    let report = evaluate(&args.repo, &catalog_path, &cluster_filter)
        .with_context(|| format!("scoring {}", args.repo.display()))?;

    let rows = collect_fix_rows(&report);
    let limit = args.limit.max(1);
    let truncated = rows.len().saturating_sub(limit);
    let rendered = rows.iter().take(limit).cloned().collect::<Vec<FixRow>>();

    let markdown = render_markdown(&report, &rendered, truncated);

    match &args.output {
        Some(path) => {
            std::fs::write(path, &markdown)
                .with_context(|| format!("writing fix list to {}", path.display()))?;
        }
        None => {
            print!("{markdown}");
        }
    }
    Ok(())
}

/// One fix-list row — the smallest atomic unit the cockpit dashboard
/// consumes. Sorted by `(score ASC, cluster ASC, pillar_id ASC)` before
/// truncation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixRow {
    /// Cluster id (e.g. "C03").
    pub cluster: String,
    /// Pillar id within the cluster (e.g. "L21"). Empty for the
    /// aggregate pillar when a cluster has no enumerated sub-pillars.
    pub pillar_id: String,
    /// Score 0..=3 for the pillar.
    pub score: u32,
    /// Gap text copied from the scorecard (e.g. "missing AGENTS.md").
    pub gap: String,
    /// Effort label parsed from the scorecard's gap string ("S", "M", "L").
    /// Defaults to "M" when the scorecard omits the marker.
    pub effort: char,
}

/// Walk the scorecard and extract every gap (one row per gap entry).
/// Clusters without `scorecard.pillars[].gaps` still produce a row
/// when the cluster score is low — the row carries the cluster-level
/// "no evidence" tag instead of a per-pillar gap line.
pub fn collect_fix_rows(report: &ScoreReport) -> Vec<FixRow> {
    let mut out = Vec::new();
    for cluster in &report.clusters {
        for pillar in &cluster.pillars {
            if pillar.gaps.is_empty() {
                if pillar.score == 0 {
                    // Score-zero pillar with no enumerated gaps — emit a
                    // synthetic row so the fix-list still surfaces it.
                    out.push(FixRow {
                        cluster: cluster.cluster.clone(),
                        pillar_id: pillar.pillar_id.clone(),
                        score: pillar.score,
                        gap: "(no evidence found)".into(),
                        effort: 'M',
                    });
                }
                continue;
            }
            for gap in &pillar.gaps {
                // Scorecard gap text is rendered like
                //   "Has cargo-deny configured — effort: S"
                // Pull the trailing "effort: <X>" marker out; default
                // to 'M' when absent.
                let (text, effort) = parse_effort(gap);
                out.push(FixRow {
                    cluster: cluster.cluster.clone(),
                    pillar_id: pillar.pillar_id.clone(),
                    score: pillar.score,
                    gap: text,
                    effort,
                });
            }
        }
        // When the cluster has no pillars (degenerate catalog) we still
        // want a row for low scores so the fix-list isn't empty.
        if cluster.pillars.is_empty() && cluster.total_points == 0 {
            out.push(FixRow {
                cluster: cluster.cluster.clone(),
                pillar_id: String::new(),
                score: 0,
                gap: "(empty pillar set)".into(),
                effort: 'M',
            });
        }
    }
    // Sort: worst-first (ascending score), then cluster-id, then pillar-id.
    out.sort_by(|a, b| {
        a.score
            .cmp(&b.score)
            .then_with(|| a.cluster.cmp(&b.cluster))
            .then_with(|| a.pillar_id.cmp(&b.pillar_id))
    });
    out
}

/// Split a gap string into its descriptive text + trailing effort
/// marker. The current scoring-engine renders gaps as
/// `<text> — effort: <X>`; this parser is forgiving.
fn parse_effort(gap: &str) -> (String, char) {
    let lower = gap.to_ascii_lowercase();
    let needle = "effort:";
    if let Some(idx) = lower.rfind(needle) {
        let head = gap[..idx]
            .trim_end_matches(|c: char| c == '—' || c == '-' || c == ' ')
            .trim();
        let tail = &gap[idx + needle.len()..];
        let ch = tail
            .trim()
            .chars()
            .next()
            .map(|c| c.to_ascii_uppercase())
            .unwrap_or('M');
        (head.to_string(), ch)
    } else {
        (gap.trim().to_string(), 'M')
    }
}

/// Total gap count for a cluster — used by the footer summary.
fn total_gaps_for(report: &ScoreReport, cluster_id: &str) -> usize {
    report
        .clusters
        .iter()
        .find(|c| c.cluster == cluster_id)
        .map(|c| c.pillars.iter().map(|p| p.gaps.len()).sum::<usize>())
        .unwrap_or(0)
}

/// Render the markdown output. Public for unit testing.
pub fn render_markdown(report: &ScoreReport, rows: &[FixRow], truncated: usize) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Fix list — {} ({})", report.repo, report.date);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "_Top {} of {} gap{}, spread across {} cluster{}._",
        rows.len(),
        rows.len() + truncated,
        if rows.len() + truncated == 1 { "" } else { "s" },
        report.clusters.len(),
        if report.clusters.len() == 1 { "" } else { "s" },
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Ranked worst-first (ascending score, then cluster id ascending)."
    );
    let _ = writeln!(out);

    if rows.is_empty() {
        let _ = writeln!(out, "_No gaps — every cluster is at full score._");
        return out;
    }

    // Main fix-list table.
    let mut table = Table::new();
    table.set_header(["#", "Cluster", "Pillar", "Score", "Effort", "Gap"]);
    table.set_width(120);
    for (i, r) in rows.iter().enumerate() {
        table.add_row(Row::from(vec![
            Cell::new((i + 1).to_string()),
            Cell::new(r.cluster.clone()),
            Cell::new(r.pillar_id.clone()),
            Cell::new(score_glyph(r.score)),
            Cell::new(r.effort.to_string()),
            Cell::new(r.gap.clone()),
        ]));
    }
    let _ = writeln!(out, "{}", table);
    let _ = writeln!(out);

    if truncated > 0 {
        let _ = writeln!(
            out,
            "_…and {} more gap{} excluded by --limit._",
            truncated,
            if truncated == 1 { "" } else { "s" },
        );
    }

    // Per-cluster totals so the operator can pick where to invest.
    let _ = writeln!(out, "## Per-cluster gap totals");
    let _ = writeln!(out);
    let mut totals = Table::new();
    totals.set_header(["Cluster", "Score", "Max", "Gaps"]);
    for cluster in &report.clusters {
        totals.add_row(Row::from(vec![
            Cell::new(cluster.cluster.clone()),
            Cell::new(format!("{}/{}", cluster.total_points, cluster.max_points)),
            Cell::new(cluster.max_points.to_string()),
            Cell::new(total_gaps_for(report, &cluster.cluster).to_string()),
        ]));
    }
    let _ = writeln!(out, "{}", totals);
    let _ = writeln!(out);

    out
}

/// Two-character compact score glyph — re-uses the v38 scorecard
/// glyph vocabulary without pulling unicode deps into the table.
fn score_glyph(score: u32) -> &'static str {
    match score {
        0 => "0 ✗",
        1 => "1 △",
        2 => "2 ~",
        3 => "3 ✓",
        _ => "?",
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use agileplus_governance::scoring_engine::{PillarScore, ScoreReport};

    fn sample_report() -> ScoreReport {
        ScoreReport {
            repo: "demo".into(),
            date: "2026-07-06".into(),
            clusters: vec![
                ClusterScore {
                    cluster: "C03".into(),
                    pillars: vec![PillarScore {
                        pillar_id: "L21".into(),
                        title: "L21 — FR/NFR".into(),
                        score: 1,
                        glyph: "△",
                        evidence: vec!["AGENTS.md".into()],
                        gaps: vec!["FR/NFR catalog missing — effort: M".into()],
                        soft_goal_delta: "partial".into(),
                    }],
                    total_points: 1,
                    max_points: 3,
                },
                ClusterScore {
                    cluster: "C04".into(),
                    pillars: vec![
                        PillarScore {
                            pillar_id: "L31".into(),
                            title: "L31 — gitleaks".into(),
                            score: 0,
                            glyph: "✗",
                            evidence: vec![],
                            gaps: vec!["Has gitleaks config — effort: S".into()],
                            soft_goal_delta: "partial".into(),
                        },
                        PillarScore {
                            pillar_id: "L32".into(),
                            title: "L32 — trufflehog".into(),
                            score: 0,
                            glyph: "✗",
                            evidence: vec![],
                            gaps: vec!["Runs trufflehog in CI — effort: S".into()],
                            soft_goal_delta: "partial".into(),
                        },
                    ],
                    total_points: 0,
                    max_points: 6,
                },
            ],
        }
    }

    #[test]
    fn parse_effort_strips_marker_and_returns_default_when_absent() {
        let (t, e) = parse_effort("Has cargo-deny configured — effort: S");
        assert_eq!(t, "Has cargo-deny configured");
        assert_eq!(e, 'S');
        let (t, e) = parse_effort("no marker here");
        assert_eq!(t, "no marker here");
        assert_eq!(e, 'M');
    }

    #[test]
    fn collect_fix_rows_emits_one_per_gap_and_sorts_worst_first() {
        let report = sample_report();
        let rows = collect_fix_rows(&report);
        assert_eq!(rows.len(), 3, "1 (C03) + 2 (C04) = 3 gaps");

        // Worst score first (0s come before 1s). Within ties, sort by
        // cluster-id then pillar-id.
        assert_eq!(rows[0].score, 0);
        assert_eq!(rows[0].cluster, "C04");
        assert_eq!(rows[0].pillar_id, "L31");
        assert_eq!(rows[0].effort, 'S');

        assert_eq!(rows[1].score, 0);
        assert_eq!(rows[1].cluster, "C04");
        assert_eq!(rows[1].pillar_id, "L32");

        assert_eq!(rows[2].score, 1);
        assert_eq!(rows[2].cluster, "C03");
    }

    #[test]
    fn collect_fix_rows_emits_synthetic_row_for_score_zero_with_no_gaps() {
        let report = ScoreReport {
            repo: "demo".into(),
            date: "2026-07-06".into(),
            clusters: vec![ClusterScore {
                cluster: "C09".into(),
                pillars: vec![PillarScore {
                    pillar_id: "L51".into(),
                    title: "L51 — Accessibility".into(),
                    score: 0,
                    glyph: "✗",
                    evidence: vec![],
                    gaps: vec![],
                    soft_goal_delta: "partial".into(),
                }],
                total_points: 0,
                max_points: 3,
            }],
        };
        let rows = collect_fix_rows(&report);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].gap, "(no evidence found)");
        assert_eq!(rows[0].effort, 'M');
    }

    #[test]
    fn render_markdown_includes_cluster_table_and_truncation_notice() {
        let report = sample_report();
        let rows = collect_fix_rows(&report);
        let limit = 1;
        let truncated = rows.len().saturating_sub(limit);
        let md = render_markdown(
            &report,
            &rows.iter().take(limit).cloned().collect::<Vec<_>>(),
            truncated,
        );

        assert!(md.contains("# Fix list — demo"));
        assert!(md.contains("Cluster"));
        assert!(md.contains("Pillar"));
        assert!(md.contains("Effort"));
        assert!(md.contains("C04"));
        assert!(md.contains("C03")); // Per-cluster totals table contains it
        assert!(md.contains("--limit"), "expected truncation copy: {md}");
    }

    #[test]
    fn render_markdown_handles_empty_fix_list() {
        let report = ScoreReport {
            repo: "empty".into(),
            date: "2026-07-06".into(),
            clusters: vec![],
        };
        let md = render_markdown(&report, &[], 0);
        assert!(md.contains("No gaps"));
    }

    #[test]
    fn score_glyph_uses_canonical_unicode() {
        assert_eq!(score_glyph(0), "0 ✗");
        assert_eq!(score_glyph(3), "3 ✓");
        assert_eq!(score_glyph(99), "?");
    }

    #[test]
    fn total_gaps_for_returns_cluster_pillar_gap_count() {
        let report = sample_report();
        assert_eq!(total_gaps_for(&report, "C03"), 1);
        assert_eq!(total_gaps_for(&report, "C04"), 2);
        assert_eq!(total_gaps_for(&report, "C99"), 0);
    }
}
