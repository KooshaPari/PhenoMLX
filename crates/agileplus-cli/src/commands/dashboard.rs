//! `ap dashboard` subcommand â€” render an in-flight DAG view of the
//! AgilePlus SQLite database.  Aggregates:
//!
//!   1. **Work package state counts** (planned / doing / review /
//!      done / blocked) â€” kanban-style summary, rendered with a
//!      [`comfy_table::Table`] for visual alignment and a unicode
//!      `â–ˆ` bar to convey relative weight at a glance.
//!   2. **Recent worklog entries** â€” most-recent N rows from the
//!      `worklog_entries` table (L2 #39 ingest target), showing
//!      task id, status, agent, and completion timestamp.
//!   3. **Recent events** â€” most-recent N rows from the
//!      `events` table, showing entity, event_type, actor, and
//!      timestamp.
//!   4. **Trace link summary** â€” count of edges in
//!      `trace_links` (L2 #40 surface), grouped by link_type.
//!
//! Traceability: L2 #40 (V3 DAG layer 2).
//!
//! ## Usage
//!
//! ```text
//! ap dashboard [OPTIONS]
//!
//! Options:
//!       --limit <N>        How many rows to show per "recent" section.
//!                          [default: 5]
//!       --db <PATH>        SQLite database path.  [default: $AGILEPLUS_DB
//!                          or ./agileplus.db]
//!       --json             Emit the same data as a structured JSON
//!                          document instead of the ASCII table.
//!       --no-color         Suppress ANSI color escape codes in the
//!                          ASCII table output.
//! ```

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Args;
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Row, Table};
use rusqlite::{params, Connection};
use serde::Serialize;

#[cfg_attr(not(test), allow(unused_imports))]
use agileplus_domain::domain::work_package::WpState;
#[cfg_attr(not(test), allow(unused_imports))]
use agileplus_sqlite::migrations::MigrationRunner;

use crate::commands::trace::{open_db, resolve_db_path};

// â”€â”€ CLI surface â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Debug, Args)]
pub struct DashboardArgs {
    /// How many rows to show per "recent" section.
    #[arg(long, value_name = "N", default_value_t = 5)]
    pub limit: i64,

    /// SQLite database path. Defaults to `$AGILEPLUS_DB` or `./agileplus.db`.
    #[arg(long, value_name = "PATH")]
    pub db: Option<PathBuf>,

    /// Emit JSON instead of the ASCII table.
    #[arg(long)]
    pub json: bool,

    /// Suppress ANSI color codes in the table output.
    #[arg(long)]
    pub no_color: bool,
}

// â”€â”€ Aggregated view model â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Aggregated, in-flight DAG view.  All counts are computed in a single
/// pass over the SQLite database.
#[derive(Debug, Clone, Serialize)]
pub struct DashboardSnapshot {
    pub generated_at: String,
    pub db_path: String,
    pub work_packages: WpStateBreakdown,
    pub recent_worklog_entries: Vec<WorklogEntryRow>,
    pub recent_events: Vec<EventRow>,
    pub trace_link_summary: Vec<TraceLinkCount>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct WpStateBreakdown {
    pub total: i64,
    pub planned: i64,
    pub doing: i64,
    pub review: i64,
    pub done: i64,
    pub blocked: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorklogEntryRow {
    pub id: i64,
    pub task_id: String,
    pub agent_id: String,
    pub status: String,
    pub verification_status: String,
    pub completed_at: Option<String>,
    pub ingested_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventRow {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: i64,
    pub event_type: String,
    pub actor: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceLinkCount {
    pub link_type: String,
    pub count: i64,
}

// â”€â”€ Public entry point â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub fn run(args: &DashboardArgs) -> Result<()> {
    let db_path = resolve_db_path(args.db.as_deref());
    let conn = open_db(&db_path)?;
    let limit = args.limit.clamp(1, 100);

    let snapshot = collect_snapshot(&conn, &db_path, limit)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }

    print_ascii(&snapshot, args.no_color);
    Ok(())
}

/// Build the [`DashboardSnapshot`] from the given open connection.
/// Public so unit tests can exercise the aggregation logic against a
/// fixture database without going through the CLI.
pub fn collect_snapshot(
    conn: &Connection,
    db_path: &Path,
    limit: i64,
) -> Result<DashboardSnapshot> {
    let wp = load_wp_breakdown(conn)?;
    let worklogs = load_recent_worklog_entries(conn, limit)?;
    let events = load_recent_events(conn, limit)?;
    let trace = load_trace_link_counts(conn)?;

    Ok(DashboardSnapshot {
        generated_at: chrono::Utc::now().to_rfc3339(),
        db_path: db_path.display().to_string(),
        work_packages: wp,
        recent_worklog_entries: worklogs,
        recent_events: events,
        trace_link_summary: trace,
    })
}

// â”€â”€ Aggregation queries â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub(crate) fn load_wp_breakdown(conn: &Connection) -> Result<WpStateBreakdown> {
    // work_packages.state is a TEXT column with a CHECK constraint
    // mirroring `WpState`.  We aggregate with conditional sums so a
    // single round-trip returns the full breakdown.
    let mut stmt = conn.prepare(
        "SELECT \
            COUNT(*) AS total, \
            SUM(CASE WHEN state = 'planned' THEN 1 ELSE 0 END) AS planned, \
            SUM(CASE WHEN state = 'doing'   THEN 1 ELSE 0 END) AS doing, \
            SUM(CASE WHEN state = 'review'  THEN 1 ELSE 0 END) AS review, \
            SUM(CASE WHEN state = 'done'    THEN 1 ELSE 0 END) AS done, \
            SUM(CASE WHEN state = 'blocked' THEN 1 ELSE 0 END) AS blocked \
         FROM work_packages",
    )?;
    let row = stmt.query_row([], |r| {
        let total: i64 = r.get(0)?;
        // SUM returns NULL for an empty table; coalesce to 0.
        let planned: i64 = r.get::<_, Option<i64>>(1)?.unwrap_or(0);
        let doing: i64 = r.get::<_, Option<i64>>(2)?.unwrap_or(0);
        let review: i64 = r.get::<_, Option<i64>>(3)?.unwrap_or(0);
        let done: i64 = r.get::<_, Option<i64>>(4)?.unwrap_or(0);
        let blocked: i64 = r.get::<_, Option<i64>>(5)?.unwrap_or(0);
        Ok(WpStateBreakdown {
            total,
            planned,
            doing,
            review,
            done,
            blocked,
        })
    })?;
    Ok(row)
}

pub(crate) fn load_recent_worklog_entries(
    conn: &Connection,
    limit: i64,
) -> Result<Vec<WorklogEntryRow>> {
    // Tolerate a missing table (L2 #38 migrations not yet applied):
    // the worklog_entries table is consumed but not strictly required
    // for the dashboard to render.
    let table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='worklog_entries'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if table_exists == 0 {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT id, task_id, agent_id, status, verification_status, completed_at, ingested_at \
         FROM worklog_entries \
         ORDER BY id DESC \
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok(WorklogEntryRow {
                id: r.get(0)?,
                task_id: r.get(1)?,
                agent_id: r.get(2)?,
                status: r.get(3)?,
                verification_status: r.get(4)?,
                completed_at: r.get(5)?,
                ingested_at: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub(crate) fn load_recent_events(conn: &Connection, limit: i64) -> Result<Vec<EventRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, entity_id, event_type, actor, timestamp \
         FROM events \
         ORDER BY id DESC \
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok(EventRow {
                id: r.get(0)?,
                entity_type: r.get(1)?,
                entity_id: r.get(2)?,
                event_type: r.get(3)?,
                actor: r.get(4)?,
                timestamp: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub(crate) fn load_trace_link_counts(conn: &Connection) -> Result<Vec<TraceLinkCount>> {
    let table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='trace_links'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if table_exists == 0 {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT link_type, COUNT(*) AS n \
         FROM trace_links \
         GROUP BY link_type \
         ORDER BY n DESC, link_type ASC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(TraceLinkCount {
                link_type: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

// â”€â”€ ASCII rendering â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn print_ascii(snap: &DashboardSnapshot, no_color: bool) {
    let title = format!(
        "agileplus dashboard â€” generated {} â€” db: {}",
        truncate(&snap.generated_at, 19),
        snap.db_path
    );
    println!("\n{title}\n");
    println!("{}", "-".repeat(title.len().max(60)));

    render_wp_section(&snap.work_packages, no_color);
    render_worklog_section(&snap.recent_worklog_entries, no_color);
    render_events_section(&snap.recent_events, no_color);
    render_trace_links_section(&snap.trace_link_summary, no_color);

    println!();
}

fn render_wp_section(wp: &WpStateBreakdown, no_color: bool) {
    println!("\n[ Work packages by state ]  total = {}", wp.total);
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(Row::from(vec![
            hcell("STATE", no_color),
            hcell("COUNT", no_color),
            hcell("BAR", no_color),
        ]));

    let max_count = wp
        .planned
        .max(wp.doing)
        .max(wp.review)
        .max(wp.done)
        .max(wp.blocked)
        .max(1);
    let bar_width: usize = 24;
    let push = |table: &mut Table, label: &str, count: i64, color: Color| {
        let bar = build_bar(count, max_count, bar_width);
        let mut cell = Cell::new(format!("{count:<5}  {bar}"));
        if !no_color {
            cell = cell.fg(color);
        }
        let mut label_cell = Cell::new(label);
        if !no_color {
            label_cell = label_cell.add_attribute(Attribute::Bold).fg(color);
        }
        table.add_row(Row::from(vec![label_cell, cell]));
    };
    push(&mut table, "planned", wp.planned, Color::White);
    push(&mut table, "doing", wp.doing, Color::Yellow);
    push(&mut table, "review", wp.review, Color::Cyan);
    push(&mut table, "done", wp.done, Color::Green);
    push(&mut table, "blocked", wp.blocked, Color::Red);

    println!("{table}");
}

fn render_worklog_section(rows: &[WorklogEntryRow], no_color: bool) {
    println!("\n[ Recent worklog entries ]  ({} shown)", rows.len());
    if rows.is_empty() {
        println!("    <no worklog entries ingested yet>");
        return;
    }
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(Row::from(vec![
            hcell("TASK", no_color),
            hcell("STATUS", no_color),
            hcell("VERIFY", no_color),
            hcell("AGENT", no_color),
            hcell("COMPLETED", no_color),
        ]));
    for r in rows {
        table.add_row(Row::from(vec![
            Cell::new(&r.task_id),
            colorize_status(&r.status, no_color),
            colorize_status(&r.verification_status, no_color),
            Cell::new(truncate(&r.agent_id, 20)),
            Cell::new(
                r.completed_at
                    .as_deref()
                    .map(|s| truncate(s, 19))
                    .unwrap_or_else(|| "â€”".to_string()),
            ),
        ]));
    }
    println!("{table}");
}

fn render_events_section(rows: &[EventRow], no_color: bool) {
    println!("\n[ Recent events ]  ({} shown)", rows.len());
    if rows.is_empty() {
        println!("    <no events recorded>");
        return;
    }
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(Row::from(vec![
            hcell("ID", no_color),
            hcell("ENTITY", no_color),
            hcell("EVENT", no_color),
            hcell("ACTOR", no_color),
            hcell("TIMESTAMP", no_color),
        ]));
    for r in rows {
        let entity = format!("{}:{}", r.entity_type, r.entity_id);
        table.add_row(Row::from(vec![
            Cell::new(r.id),
            Cell::new(truncate(&entity, 24)),
            Cell::new(truncate(&r.event_type, 24)),
            Cell::new(truncate(&r.actor, 20)),
            Cell::new(truncate(&r.timestamp, 19)),
        ]));
    }
    println!("{table}");
}

fn render_trace_links_section(rows: &[TraceLinkCount], no_color: bool) {
    println!(
        "\n[ Trace links by type ]  total = {}",
        rows.iter().map(|r| r.count).sum::<i64>()
    );
    if rows.is_empty() {
        println!("    <no trace links recorded>");
        return;
    }
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(Row::from(vec![
            hcell("LINK TYPE", no_color),
            hcell("COUNT", no_color),
        ]));
    for r in rows {
        table.add_row(Row::from(vec![Cell::new(&r.link_type), Cell::new(r.count)]));
    }
    println!("{table}");
}

fn hcell(text: &str, no_color: bool) -> Cell {
    let cell = Cell::new(text);
    if no_color {
        cell
    } else {
        cell.add_attribute(Attribute::Bold).fg(Color::Magenta)
    }
}

fn colorize_status(s: &str, no_color: bool) -> Cell {
    let color = match s {
        "completed" | "passed" | "done" => Some(Color::Green),
        "running" | "doing" | "in_progress" | "review" => Some(Color::Yellow),
        "blocked" | "failed" => Some(Color::Red),
        "pending" | "todo" | "planned" | "not_run" => Some(Color::White),
        "cancelled" | "partial" => Some(Color::Cyan),
        _ => None,
    };
    let cell = Cell::new(s);
    if no_color {
        cell
    } else if let Some(c) = color {
        cell.fg(c)
    } else {
        cell
    }
}

fn build_bar(count: i64, max: i64, width: usize) -> String {
    if max <= 0 || count <= 0 {
        return " ".repeat(width);
    }
    let filled = (count as f64 / max as f64 * width as f64).round() as usize;
    let filled = filled.min(width);
    let mut s = String::with_capacity(width);
    for _ in 0..filled {
        s.push('\u{2588}'); // full block
    }
    for _ in filled..width {
        s.push(' ');
    }
    s
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{t}â€¦")
}

// â”€â”€ Unit tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Build a fully-migrated, in-memory SQLite connection suitable for
    /// exercising the dashboard aggregations.
    fn make_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        MigrationRunner::new(&conn).run_all().unwrap();
        conn
    }

    /// Seed the bare minimum rows required to render every dashboard
    /// section: 1 feature, 5 work packages (one per state), 2
    /// worklog_entries rows, 1 event, 1 trace_link.
    fn seed_minimal(conn: &Connection) {
        conn.execute_batch(
            "INSERT INTO features (id, slug, friendly_name, spec_hash, state, created_at, updated_at) \
             VALUES (1, 'feat-x', 'Feat X', x'00', 'specified', '2026-06-11T00:00:00Z', '2026-06-11T00:00:00Z');",
        )
        .unwrap();
        let states = ["planned", "doing", "review", "done", "blocked"];
        for (i, state) in states.iter().enumerate() {
            let id = i as i64 + 1;
            conn.execute(
                "INSERT INTO work_packages (id, feature_id, title, state, sequence, file_scope, \
                 acceptance_criteria, agent_id, pr_url, pr_state, worktree_path, created_at, updated_at) \
                 VALUES (?1, 1, ?2, ?3, 1, '[]', '', NULL, NULL, NULL, NULL, \
                         '2026-06-11T00:00:00Z', '2026-06-11T00:00:00Z')",
                rusqlite::params![id, format!("wp-{}", id), state],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO worklog_entries (task_id, agent_id, status, commit_sha, files_changed_json, \
             verification_status, verification_notes, verification_cmds, started_at, completed_at, ingested_at) \
             VALUES ('L2-40', 'l2-subagent-40', 'completed', NULL, '[]', 'passed', '', '[]', \
                     '2026-06-11T00:00:00Z', '2026-06-11T00:30:00Z', '2026-06-11T00:31:00Z')",
            (),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO events (entity_type, entity_id, event_type, payload, actor, timestamp, \
             prev_hash, hash, sequence) \
             VALUES ('work_package', 1, 'created', '{}', 'tester', '2026-06-11T00:00:00Z', \
                     x'00', x'00', 1)",
            (),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO trace_links (from_kind, from_id, to_kind, to_id, link_type, note, \
             created_by, created_at) \
             VALUES ('work_package', '1', 'feature', '1', 'implements', '', 'tester', \
                     '2026-06-11T00:00:00Z')",
            (),
        )
        .unwrap();
    }

    #[test]
    fn wp_breakdown_aggregates_by_state() {
        let conn = make_conn();
        seed_minimal(&conn);
        let wp = load_wp_breakdown(&conn).unwrap();
        assert_eq!(wp.total, 5);
        assert_eq!(wp.planned, 1);
        assert_eq!(wp.doing, 1);
        assert_eq!(wp.review, 1);
        assert_eq!(wp.done, 1);
        assert_eq!(wp.blocked, 1);
    }

    #[test]
    fn wp_breakdown_empty_table_returns_zeros() {
        let conn = make_conn();
        let wp = load_wp_breakdown(&conn).unwrap();
        assert_eq!(wp.total, 0);
        assert_eq!(wp.planned, 0);
    }

    #[test]
    fn worklog_section_returns_recent_rows() {
        let conn = make_conn();
        seed_minimal(&conn);
        let rows = load_recent_worklog_entries(&conn, 5).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].task_id, "L2-40");
        assert_eq!(rows[0].status, "completed");
        assert_eq!(rows[0].verification_status, "passed");
    }

    #[test]
    fn events_section_returns_recent_rows() {
        let conn = make_conn();
        seed_minimal(&conn);
        let rows = load_recent_events(&conn, 5).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entity_type, "work_package");
        assert_eq!(rows[0].event_type, "created");
    }

    #[test]
    fn trace_link_section_groups_by_type() {
        let conn = make_conn();
        seed_minimal(&conn);
        // add a second link of the same type to test grouping
        conn.execute(
            "INSERT INTO trace_links (from_kind, from_id, to_kind, to_id, link_type, note, \
             created_by, created_at) \
             VALUES ('work_package', '2', 'feature', '1', 'implements', '', 'tester', \
                     '2026-06-11T00:00:00Z')",
            (),
        )
        .unwrap();
        let rows = load_trace_link_counts(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].link_type, "implements");
        assert_eq!(rows[0].count, 2);
    }

    #[test]
    fn snapshot_aggregates_every_section() {
        let conn = make_conn();
        seed_minimal(&conn);
        let snap = collect_snapshot(&conn, &PathBuf::from(":memory:"), 5).unwrap();
        assert_eq!(snap.work_packages.total, 5);
        assert_eq!(snap.recent_worklog_entries.len(), 1);
        assert_eq!(snap.recent_events.len(), 1);
        assert_eq!(snap.trace_link_summary.len(), 1);
    }

    #[test]
    fn build_bar_renders_full_block_at_max() {
        let bar = build_bar(10, 10, 8);
        assert_eq!(bar.chars().count(), 8);
        assert!(bar.chars().all(|c| c == '\u{2588}'));
    }

    #[test]
    fn build_bar_renders_zero_width_at_zero_count() {
        let bar = build_bar(0, 10, 8);
        assert_eq!(bar.chars().count(), 8);
        assert!(bar.chars().all(|c| c == ' '));
    }

    #[test]
    fn build_bar_renders_proportional_fill() {
        let bar = build_bar(5, 10, 10);
        // 5/10 * 10 = 5 blocks, 5 spaces
        let filled = bar.chars().filter(|c| *c == '\u{2588}').count();
        let spaces = bar.chars().filter(|c| *c == ' ').count();
        assert_eq!(filled, 5);
        assert_eq!(spaces, 5);
    }

    #[test]
    fn truncate_keeps_short_strings() {
        assert_eq!(truncate("abc", 5), "abc");
    }

    #[test]
    fn truncate_shortens_long_strings() {
        assert_eq!(truncate("abcdefgh", 5), "abcdâ€¦");
    }

    #[test]
    fn wp_state_enum_values_match_sql_check() {
        // Defensive: every WpState variant must map to a valid SQL state
        // string.  This protects against drift between domain enum
        // and the SQL CHECK constraint in migration 002.
        let pairs: [(WpState, &str); 5] = [
            (WpState::Planned, "planned"),
            (WpState::Doing, "doing"),
            (WpState::Review, "review"),
            (WpState::Done, "done"),
            (WpState::Blocked, "blocked"),
        ];
        for (state, sql) in pairs {
            assert_eq!(
                serde_json::to_string(&state).unwrap().trim_matches('"'),
                sql,
                "WpState::{state:?} does not serialize to `{sql}`"
            );
        }
    }
}
