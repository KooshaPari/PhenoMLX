// SPDX-License-Identifier: MIT OR Apache-2.0
//! `agileplus import-dagctl` command implementation.
//!
//! Migrates a dagctl-style SQLite database (`MELOSVIZ_DAG.db`,
//! `FLEET_DAG_v3.db`, or any `dagctl` artifact) into an AgilePlus SQLite
//! database. Maps the dagctl `tasks` + `edges` tables onto the AgilePlus
//! `work_packages` + `wp_dependencies` tables.
//!
//! ## Source schema (dagctl)
//!
//! ```sql
//! CREATE TABLE tasks (
//!     id              TEXT PRIMARY KEY,
//!     stage           INTEGER,
//!     slot            INTEGER,
//!     description     TEXT,
//!     repo            TEXT,
//!     subproject      TEXT,
//!     category        TEXT,
//!     lane            TEXT,
//!     branch          TEXT,
//!     status          TEXT,    -- 'ready' | 'in_progress' | 'done' | 'merged' | ...
//!     kind            TEXT,
//!     priority        INTEGER,
//!     semantic_hash   TEXT,
//!     side_dag        TEXT,
//!     assigned_agent  TEXT
//! );
//!
//! CREATE TABLE edges (
//!     from_task TEXT NOT NULL,
//!     to_task   TEXT NOT NULL,
//!     PRIMARY KEY (from_task, to_task)
//! );
//! ```
//!
//! ## Dest schema (AgilePlus)
//!
//! ```sql
//! CREATE TABLE work_packages (
//!     id          INTEGER PRIMARY KEY AUTOINCREMENT,
//!     feature_id  INTEGER NOT NULL REFERENCES features(id) ON DELETE CASCADE,
//!     title       TEXT    NOT NULL,
//!     state       TEXT    NOT NULL CHECK(state IN ('planned','doing','review','done','blocked')),
//!     sequence    INTEGER NOT NULL DEFAULT 0,
//!     file_scope  TEXT    NOT NULL DEFAULT '[]',
//!     acceptance_criteria TEXT NOT NULL DEFAULT '',
//!     agent_id    TEXT,
//!     pr_url      TEXT,
//!     pr_state    TEXT,
//!     worktree_path TEXT,
//!     created_at  TEXT    NOT NULL,
//!     updated_at  TEXT    NOT NULL
//! );
//!
//! CREATE TABLE wp_dependencies (
//!     wp_id      INTEGER NOT NULL REFERENCES work_packages(id) ON DELETE CASCADE,
//!     depends_on INTEGER NOT NULL REFERENCES work_packages(id) ON DELETE CASCADE,
//!     dep_type   TEXT    NOT NULL CHECK(dep_type IN ('explicit','file_overlap','data')),
//!     PRIMARY KEY (wp_id, depends_on)
//! );
//! ```
//!
//! ## Mapping
//!
//! | dagctl | AgilePlus |
//! |--------|-----------|
//! | `tasks.description` (truncated to 200ch) | `work_packages.title` |
//! | `tasks.description` | `work_packages.acceptance_criteria` |
//! | `tasks.status` (`ready/in_progress/done/...`) | mapped â†’ `planned/doing/review/done/blocked` |
//! | `tasks.id` | kept as a side-band string in `file_scope` JSON to avoid an extra migration |
//! | `tasks.subproject` | side-band in `file_scope` |
//! | `tasks.stage` | `sequence` (0-based stage ordinal) |
//! | `edges.from_task â†’ to_task` | `wp_dependencies(wp_id â†’ depends_on, dep_type='explicit')` |
//!
//! Traceability: FR-AGP-023 (dagctl import).

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use rusqlite::{params, Connection};
use serde_json::json;

/// Arguments for `agileplus import-dagctl`.
#[derive(Debug, Args)]
pub struct ImportDagctlArgs {
    /// Path to the source dagctl SQLite database (e.g. `MELOSVIZ_DAG.db`).
    #[arg(long)]
    pub from: PathBuf,

    /// Path to the dest AgilePlus SQLite database (created if missing).
    #[arg(long, default_value = "agileplus.db")]
    pub db: PathBuf,

    /// Optional slug for a feature to attach the imported WPs to.
    /// If the feature doesn't exist, it is created with this slug.
    #[arg(long, default_value = "imported-from-dagctl")]
    pub feature_slug: String,

    /// Friendly name for the created/used feature.
    #[arg(long, default_value = "Imported from dagctl")]
    pub feature_name: String,

    /// Print a per-row migration report.
    #[arg(long)]
    pub verbose: bool,

    /// Dry run: report counts but make no DB writes to dest.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: &ImportDagctlArgs) -> Result<()> {
    // 1. Open source.
    let src = Connection::open(&args.from)
        .with_context(|| format!("opening source dagctl db: {}", args.from.display()))?;
    src.execute_batch("PRAGMA foreign_keys=ON;")
        .context("PRAGMA foreign_keys=ON (src)")?;

    let src_count: i64 = src
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
        .context("counting src tasks")?;
    let edge_count: i64 = src
        .query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))
        .context("counting src edges")?;
    eprintln!("source: {src_count} tasks, {edge_count} edges");

    if args.dry_run {
        eprintln!("dry run â€” not writing to {}", args.db.display());
        return Ok(());
    }

    // 2. Open dest + run migrations.
    let dest = Connection::open(&args.db)
        .with_context(|| format!("opening dest db: {}", args.db.display()))?;
    dest.execute_batch("PRAGMA foreign_keys=ON;")
        .context("PRAGMA foreign_keys=ON (dest)")?;
    let runner = agileplus_sqlite::migrations::MigrationRunner::new(&dest);
    runner.run_all().context("running migrations")?;

    // 3. Ensure the feature exists.
    let feature_id = ensure_feature(&dest, &args.feature_slug, &args.feature_name)?;
    eprintln!("feature_id = {feature_id}");

    // 4. Migrate tasks â†’ work_packages (id-mapping for the dep re-encoding).
    let tx = dest.unchecked_transaction()?;
    let mut id_map: HashMap<String, i64> = HashMap::new();
    let now = chrono::Utc::now().to_rfc3339();
    let mut stmt = tx.prepare(
        "INSERT INTO work_packages
            (feature_id, title, state, sequence, file_scope, acceptance_criteria,
             agent_id, worktree_path, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )?;
    let mut src_stmt = src.prepare(
        "SELECT id, description, status, stage, subproject, assigned_agent, side_dag, repo, category, kind, priority
         FROM tasks",
    )?;
    let mut imported = 0u64;
    let mut rows = src_stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let description: String = row.get(1).unwrap_or_default();
        let status: String = row.get(2).unwrap_or_else(|_| "ready".to_string());
        let stage: i64 = row.get(3).unwrap_or(0);
        let subproject: String = row.get(4).unwrap_or_default();
        let agent: Option<String> = row.get(5).ok();
        let side_dag: Option<String> = row.get(6).ok();
        let repo: String = row.get(7).unwrap_or_default();
        let category: String = row.get(8).unwrap_or_default();
        let kind: String = row.get(9).unwrap_or_default();
        let priority: i64 = row.get(10).unwrap_or(5);
        let mapped_state = map_state_owned(&status);
        // Derive a title from the description (truncate at 200ch on first sentence or newline).
        let title = derive_title(&description, &id);
        let side_band = json!({
            "dagctl_id": id,
            "subproject": subproject,
            "side_dag": side_dag,
            "repo": repo,
            "category": category,
            "kind": kind,
            "priority": priority,
        })
        .to_string();
        let wp_id: i64 = stmt.insert(params![
            feature_id,
            title,
            mapped_state,
            stage,
            side_band,
            description,
            agent,
            Option::<String>::None, // worktree_path
            now,
            now,
        ])?;
        id_map.insert(id, wp_id);
        imported += 1;
        if args.verbose {
            eprintln!("  task {imported:>5}: â†’ wp#{wp_id} ({title})");
        }
    }
    drop(rows);
    drop(src_stmt);
    drop(stmt);
    eprintln!("imported {imported} tasks â†’ work_packages");

    // 5. Migrate edges â†’ wp_dependencies.
    let mut dep_stmt = tx.prepare(
        "INSERT OR IGNORE INTO wp_dependencies (wp_id, depends_on, dep_type)
         VALUES (?, ?, ?)",
    )?;
    let mut edges_stmt = src.prepare("SELECT from_task, to_task FROM edges")?;
    let mut edges = 0u64;
    let mut missing = 0u64;
    let mut edge_rows = edges_stmt.query([])?;
    while let Some(row) = edge_rows.next()? {
        let from: String = row.get(0)?;
        let to: String = row.get(1)?;
        if let (Some(&wp_id), Some(&dep_on)) = (id_map.get(&from), id_map.get(&to)) {
            dep_stmt.execute(params![wp_id, dep_on, "explicit"])?;
            edges += 1;
            if args.verbose {
                eprintln!("  edge {edges:>5}: {from} â†’ {to} = wp#{wp_id} â†� wp#{dep_on}");
            }
        } else {
            missing += 1;
        }
    }
    drop(edge_rows);
    drop(edges_stmt);
    drop(dep_stmt);
    eprintln!("imported {edges} edges â†’ wp_dependencies ({missing} dangling skipped)");

    tx.commit().context("committing transaction")?;

    eprintln!(
        "âœ“ import-dagctl complete: {imported} work_packages + {edges} wp_dependencies into {}",
        args.db.display()
    );
    Ok(())
}

fn ensure_feature(conn: &Connection, slug: &str, name: &str) -> Result<i64> {
    // Try to find an existing feature with this slug.
    let found: Option<i64> = conn
        .query_row("SELECT id FROM features WHERE slug = ?", [slug], |r| {
            r.get(0)
        })
        .ok();
    if let Some(id) = found {
        return Ok(id);
    }
    // Create a new one.
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO features (slug, friendly_name, spec_hash, state, created_at, updated_at)
         VALUES (?, ?, ?, 'specified', ?, ?)",
        rusqlite::params![slug, name, vec![0u8; 32].as_slice(), now, now],
    )
    .context("inserting feature row")?;
    let id: i64 = conn.query_row("SELECT id FROM features WHERE slug = ?", [slug], |r| {
        r.get(0)
    })?;
    Ok(id)
}

fn map_state(s: &str) -> &'static str {
    match s {
        "ready" | "pending" | "planned" | "open" => "planned",
        "doing" | "in_progress" | "wip" => "doing",
        "review" | "reviewing" | "pr" => "review",
        "done" | "completed" | "shipped" | "merged" => "done",
        "blocked" | "stalled" => "blocked",
        _ => "planned",
    }
}

fn map_state_owned(s: &str) -> String {
    map_state(s).to_string()
}

/// Allowed values for `wp_dependencies.dep_type` per `008_create_wp_dependencies.sql`.
/// Kept here for future use (e.g. when the dagctl edge format adds `dep_type`).
#[allow(dead_code)] // reserved - dep_type mapping for when dagctl adds dep_type field
fn map_dep_type(s: &str) -> &'static str {
    match s {
        "file_overlap" => "file_overlap",
        "data" => "data",
        _ => "explicit",
    }
}

#[allow(dead_code)] // reserved - dep_type mapping for when dagctl adds dep_type field
fn map_dep_type_owned(s: &str) -> String {
    map_dep_type(s).to_string()
}

/// Derive a (â‰¤200ch) work-package title from the dagctl description.
/// Strategy: take the first sentence (up to first `.`, `:` or newline),
/// collapse whitespace, then truncate to 200 chars. Falls back to the dagctl id.
fn derive_title(description: &str, fallback_id: &str) -> String {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return fallback_id.to_string();
    }
    // Find first sentence boundary.
    let cut = trimmed.find(['.', ':', '\n']).unwrap_or(trimmed.len());
    let first = &trimmed[..cut];
    // Collapse whitespace.
    let collapsed: String = first.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return fallback_id.to_string();
    }
    if collapsed.len() > 200 {
        format!("{}â€¦", &collapsed[..199])
    } else {
        collapsed
    }
}

// â”€â”€ Tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_state_covers_all_dagctl_states() {
        assert_eq!(map_state("ready"), "planned");
        assert_eq!(map_state("in_progress"), "doing");
        assert_eq!(map_state("review"), "review");
        assert_eq!(map_state("done"), "done");
        assert_eq!(map_state("blocked"), "blocked");
        assert_eq!(map_state("merged"), "done");
        assert_eq!(map_state("unknown_state"), "planned"); // default
    }

    #[test]
    fn map_dep_type_passes_through_known_kinds() {
        assert_eq!(map_dep_type("explicit"), "explicit");
        assert_eq!(map_dep_type("file_overlap"), "file_overlap");
        assert_eq!(map_dep_type("data"), "data");
        assert_eq!(map_dep_type("garbage"), "explicit");
    }

    #[test]
    fn derive_title_truncates_and_uses_fallback() {
        assert_eq!(derive_title("", "task-1"), "task-1");
        let long = "x".repeat(500);
        let t = derive_title(&long, "task-1");
        assert_eq!(t.chars().count(), 200);
        assert!(t.ends_with('â€¦'));
        assert_eq!(
            derive_title("First sentence. Second one.", "fb"),
            "First sentence"
        );
        assert_eq!(
            derive_title("Just a title without period", "fb"),
            "Just a title without period"
        );
        assert_eq!(
            derive_title("With colon: more text\nignored", "fb"),
            "With colon"
        );
    }
}
