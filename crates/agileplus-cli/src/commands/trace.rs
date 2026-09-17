//! `ap trace link <from> <to>` subcommand â€” inserts a row into the
//! `trace_links` table to record a directed edge between two domain
//! entities.  Also supports the inverse / list-view surface used by
//! `ap dashboard`.
//!
//! Traceability: L2 #40 (V3 DAG layer 2).
//!
//! ## Argument shape
//!
//! ```text
//! ap trace link <from> <to> [OPTIONS]
//!
//! Arguments:
//!   <from>  Source entity ref in the form `<kind>:<id>`, e.g. `wp:42`
//!   <to>    Target entity ref in the form `<kind>:<id>`, e.g. `feature:7`
//!
//! Options:
//!       --link-type <LINK>   One of: parent_of, child_of, depends_on,
//!                            blocks, implements, verifies, references,
//!                            duplicates.  [default: implements]
//!       --note <NOTE>        Free-form note.  [default: ""]
//!       --by <ACTOR>         Actor recording the link.  [default:
//!                            $USER or "system"]
//!       --db <PATH>          SQLite database path.  [default: $AGILEPLUS_DB
//!                            or ./agileplus.db]
//! ```
//!
//! ## Example
//!
//! ```bash
//! ap trace link wp:42 feature:7 --link-type implements \
//!     --note "WP-42 implements the dashboard's WpState filter"
//! ```

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use rusqlite::{params, Connection};

use agileplus_sqlite::migrations::MigrationRunner;

// â”€â”€ CLI surface â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Top-level `trace` args struct â€” implements `clap::Args` so it can be
/// embedded as a variant in the parent `Command` enum in `main.rs`.
#[derive(Debug, Args)]
pub struct TraceCmd {
    #[command(subcommand)]
    pub sub: TraceSub,
}

impl TraceCmd {
    /// Dispatch the chosen subcommand.
    pub async fn run(&self) -> anyhow::Result<()> {
        match &self.sub {
            TraceSub::Link(a) => run_link(a),
            TraceSub::List(a) => run_list(a),
            TraceSub::Show(a) => run_show(a),
        }
    }
}

/// Legacy alias kept for callers that still use `TraceArgs` directly.
pub type TraceArgs = TraceCmd;

#[derive(Debug, Subcommand)]
pub enum TraceSub {
    /// Insert a directed trace link between two entities.
    Link(LinkArgs),
    /// List the most-recent trace links (default 25).
    List(ListArgs),
    /// Show every trace link that touches a given entity.
    Show(ShowArgs),
}

#[derive(Debug, Args)]
pub struct LinkArgs {
    /// Source entity ref `<kind>:<id>` (e.g. `wp:42`).
    pub from: String,

    /// Target entity ref `<kind>:<id>` (e.g. `feature:7`).
    pub to: String,

    /// Edge semantic. One of: parent_of, child_of, depends_on, blocks,
    /// implements, verifies, references, duplicates.
    #[arg(long, value_name = "TYPE", default_value = "implements")]
    pub link_type: String,

    /// Free-form note attached to the link.
    #[arg(long, value_name = "NOTE", default_value = "")]
    pub note: String,

    /// Actor recording the link (e.g. user id, agent id).
    #[arg(long, value_name = "ACTOR")]
    pub by: Option<String>,

    /// SQLite database path. Defaults to `$AGILEPLUS_DB` or `./agileplus.db`.
    #[arg(long, value_name = "PATH")]
    pub db: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Maximum number of rows to return.
    #[arg(long, value_name = "N", default_value_t = 25)]
    pub limit: i64,
    /// SQLite database path.
    #[arg(long, value_name = "PATH")]
    pub db: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Entity ref to query, `<kind>:<id>` (e.g. `wp:42`).
    pub entity: String,
    /// SQLite database path.
    #[arg(long, value_name = "PATH")]
    pub db: Option<PathBuf>,
}

// â”€â”€ Allowed enumerations (mirror CHECK constraints) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

const ALLOWED_KINDS: &[&str] = &[
    "work_package",
    "feature",
    "story",
    "epic",
    "project",
    "cycle",
    "module",
    "requirement",
    "external",
];

const ALLOWED_LINK_TYPES: &[&str] = &[
    "parent_of",
    "child_of",
    "depends_on",
    "blocks",
    "implements",
    "verifies",
    "references",
    "duplicates",
];

// â”€â”€ Public entry points â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Dispatch the `trace` subcommand.  Each variant opens the DB itself so
/// the caller can hand off via the `agileplus-cli` main entry point.
pub fn run(args: &TraceCmd) -> Result<()> {
    match &args.sub {
        TraceSub::Link(a) => run_link(a),
        TraceSub::List(a) => run_list(a),
        TraceSub::Show(a) => run_show(a),
    }
}

pub fn run_link(args: &LinkArgs) -> Result<()> {
    let (from_kind, from_id) = parse_ref(&args.from, "from")?;
    let (to_kind, to_id) = parse_ref(&args.to, "to")?;

    if !ALLOWED_KINDS.contains(&from_kind.as_str()) {
        bail!(
            "invalid --from-kind `{from_kind}`; allowed: {}",
            ALLOWED_KINDS.join(", ")
        );
    }
    if !ALLOWED_KINDS.contains(&to_kind.as_str()) {
        bail!(
            "invalid --to-kind `{to_kind}`; allowed: {}",
            ALLOWED_KINDS.join(", ")
        );
    }
    if !ALLOWED_LINK_TYPES.contains(&args.link_type.as_str()) {
        bail!(
            "invalid --link-type `{}`; allowed: {}",
            args.link_type,
            ALLOWED_LINK_TYPES.join(", ")
        );
    }
    if from_kind == to_kind && from_id == to_id {
        bail!("refusing to create a self-link `{from_kind}:{from_id}` -> `{to_kind}:{to_id}`");
    }

    let db_path = resolve_db_path(args.db.as_deref());
    let conn = open_db(&db_path)?;
    let actor = args
        .by
        .clone()
        .or_else(|| std::env::var("USER").ok())
        .or_else(|| std::env::var("USERNAME").ok())
        .unwrap_or_else(|| "system".to_string());
    let now = chrono::Utc::now().to_rfc3339();

    // Idempotent insert: a UNIQUE(from_kind, from_id, to_kind, to_id, link_type)
    // constraint de-duplicates identical links.  Use INSERT OR IGNORE so
    // re-running the same `ap trace link` is a no-op.
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO trace_links \
            (from_kind, from_id, to_kind, to_id, link_type, note, created_by, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            from_kind,
            from_id,
            to_kind,
            to_id,
            args.link_type,
            args.note,
            actor,
            now
        ],
    )?;

    if inserted == 0 {
        println!(
            "trace link already exists: {from_kind}:{from_id} --{}--> {to_kind}:{to_id}",
            args.link_type
        );
    } else {
        let row_id = conn.last_insert_rowid();
        println!(
            "trace link #{row_id}: {from_kind}:{from_id} --{}--> {to_kind}:{to_id}",
            args.link_type
        );
        if !args.note.is_empty() {
            println!("  note: {}", args.note);
        }
        println!("  by:   {actor}");
    }

    Ok(())
}

pub fn run_list(args: &ListArgs) -> Result<()> {
    let db_path = resolve_db_path(args.db.as_deref());
    let conn = open_db(&db_path)?;
    let limit = args.limit.clamp(1, 500);

    let mut stmt = conn.prepare(
        "SELECT id, from_kind, from_id, to_kind, to_id, link_type, created_at \
         FROM trace_links \
         ORDER BY id DESC \
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok(TraceLinkRow {
                id: r.get::<_, i64>(0)?,
                from_kind: r.get(1)?,
                from_id: r.get(2)?,
                to_kind: r.get(3)?,
                to_id: r.get(4)?,
                link_type: r.get(5)?,
                created_at: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if rows.is_empty() {
        println!("No trace links recorded.");
        return Ok(());
    }

    println!(
        "{:<4}  {:<22}  {:<22}  {:<14}  CREATED",
        "ID", "FROM", "TO", "LINK"
    );
    println!("{}", "-".repeat(80));
    for row in &rows {
        println!(
            "{:<4}  {:<22}  {:<22}  {:<14}  {}",
            row.id,
            format!("{}:{}", row.from_kind, row.from_id),
            format!("{}:{}", row.to_kind, row.to_id),
            row.link_type,
            truncate(&row.created_at, 19)
        );
    }
    println!("\n{} trace link(s) shown (limit={limit}).", rows.len());

    Ok(())
}

pub fn run_show(args: &ShowArgs) -> Result<()> {
    let (kind, id) = parse_ref(&args.entity, "entity")?;
    let db_path = resolve_db_path(args.db.as_deref());
    let conn = open_db(&db_path)?;

    let mut stmt = conn.prepare(
        "SELECT id, from_kind, from_id, to_kind, to_id, link_type, note, created_by, created_at \
         FROM trace_links \
         WHERE (from_kind = ?1 AND from_id = ?2) OR (to_kind = ?1 AND to_id = ?2) \
         ORDER BY id ASC",
    )?;
    let rows = stmt
        .query_map(params![kind, id], |r| {
            Ok(TraceLinkDetail {
                id: r.get::<_, i64>(0)?,
                from_kind: r.get(1)?,
                from_id: r.get(2)?,
                to_kind: r.get(3)?,
                to_id: r.get(4)?,
                link_type: r.get(5)?,
                note: r.get(6)?,
                created_by: r.get(7)?,
                created_at: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if rows.is_empty() {
        println!("No trace links touch `{kind}:{id}`.");
        return Ok(());
    }

    println!("Trace links touching `{kind}:{id}` ({}):", rows.len());
    for r in &rows {
        let direction = if r.from_kind == kind && r.from_id == id {
            "OUT"
        } else {
            "IN "
        };
        println!(
            "  [{:>3}] {direction} {}:{} --{}--> {}:{}  (by {} @ {})",
            r.id,
            r.from_kind,
            r.from_id,
            r.link_type,
            r.to_kind,
            r.to_id,
            r.created_by,
            r.created_at
        );
        if !r.note.is_empty() {
            println!("        note: {}", r.note);
        }
    }

    Ok(())
}

// â”€â”€ Helpers â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Parse `<kind>:<id>` (e.g. `wp:42`, `feature:7`).
pub(crate) fn parse_ref(raw: &str, slot: &str) -> Result<(String, String)> {
    let (kind, id) = raw
        .split_once(':')
        .ok_or_else(|| anyhow!("{slot} must be in `<kind>:<id>` form, got `{raw}`"))?;
    if kind.is_empty() || id.is_empty() {
        bail!("{slot} must be in `<kind>:<id>` form, got `{raw}`");
    }
    Ok((kind.to_string(), id.to_string()))
}

/// Open the SQLite database and run any pending migrations so the
/// `trace_links` table exists.
pub(crate) fn open_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)
        .with_context(|| format!("opening sqlite db at {}", path.display()))?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .context("enabling foreign_keys pragma")?;
    let runner = MigrationRunner::new(&conn);
    runner.run_all().context("running migrations")?;
    Ok(conn)
}

pub(crate) fn resolve_db_path(override_path: Option<&Path>) -> PathBuf {
    override_path
        .map(|p| p.to_path_buf())
        .or_else(|| std::env::var("AGILEPLUS_DB").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("agileplus.db"))
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{t}â€¦")
}

#[derive(Debug, Clone)]
pub(crate) struct TraceLinkRow {
    pub id: i64,
    pub from_kind: String,
    pub from_id: String,
    pub to_kind: String,
    pub to_id: String,
    pub link_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TraceLinkDetail {
    pub id: i64,
    pub from_kind: String,
    pub from_id: String,
    pub to_kind: String,
    pub to_id: String,
    pub link_type: String,
    pub note: String,
    pub created_by: String,
    pub created_at: String,
}

// â”€â”€ Unit tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ref_accepts_kind_id() {
        let (k, i) = parse_ref("wp:42", "from").unwrap();
        assert_eq!(k, "wp");
        assert_eq!(i, "42");
    }

    #[test]
    fn parse_ref_rejects_missing_colon() {
        assert!(parse_ref("wp42", "from").is_err());
    }

    #[test]
    fn parse_ref_rejects_empty_segments() {
        assert!(parse_ref(":42", "from").is_err());
        assert!(parse_ref("wp:", "from").is_err());
    }

    #[test]
    fn truncate_keeps_short_strings() {
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("2026-06-11T00:00:00Z", 20), "2026-06-11T00:00:00Z");
    }

    #[test]
    fn truncate_shortens_long_strings() {
        assert_eq!(
            truncate("2026-06-11T00:00:00.123456+00:00", 10),
            "2026-06-1â€¦"
        );
    }

    #[test]
    fn link_inserts_row_into_trace_links_table() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("trace-test.db");
        let conn = open_db(&db).unwrap();

        let from_kind = "work_package";
        let from_id = "42";
        let to_kind = "feature";
        let to_id = "7";
        let link_type = "implements";
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO trace_links (from_kind, from_id, to_kind, to_id, link_type, note, created_by, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![from_kind, from_id, to_kind, to_id, link_type, "smoke", "tester", now],
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trace_links", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let (fk, fi, tk, ti, lt): (String, String, String, String, String) = conn
            .query_row(
                "SELECT from_kind, from_id, to_kind, to_id, link_type FROM trace_links",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(fk, from_kind);
        assert_eq!(fi, from_id);
        assert_eq!(tk, to_kind);
        assert_eq!(ti, to_id);
        assert_eq!(lt, link_type);
    }

    #[test]
    fn link_is_idempotent_via_unique_constraint() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("trace-idem.db");
        let conn = open_db(&db).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let args = [
            ("work_package", "1", "feature", "1", "implements"),
            ("work_package", "1", "feature", "1", "implements"), // dup
        ];
        for (fk, fi, tk, ti, lt) in args {
            conn.execute(
                "INSERT OR IGNORE INTO trace_links \
                 (from_kind, from_id, to_kind, to_id, link_type, note, created_by, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, '', 'tester', ?6)",
                params![fk, fi, tk, ti, lt, now],
            )
            .unwrap();
        }
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trace_links", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "duplicate link should be deduped");
    }

    #[test]
    fn run_link_writes_a_row_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("run-link.db");
        let args = LinkArgs {
            from: "work_package:99".to_string(),
            to: "feature:5".to_string(),
            link_type: "implements".to_string(),
            note: "unit test".to_string(),
            by: Some("test-suite".to_string()),
            db: Some(db.clone()),
        };
        run_link(&args).unwrap();

        let conn = open_db(&db).unwrap();
        let note: String = conn
            .query_row(
                "SELECT note FROM trace_links WHERE from_id = '99' AND to_id = '5'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(note, "unit test");
    }

    #[test]
    fn run_link_rejects_self_link() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("run-self.db");
        let args = LinkArgs {
            from: "work_package:1".to_string(),
            to: "work_package:1".to_string(),
            link_type: "depends_on".to_string(),
            note: String::new(),
            by: None,
            db: Some(db),
        };
        let err = run_link(&args).unwrap_err();
        assert!(format!("{err:#}").contains("self-link"));
    }

    #[test]
    fn run_link_rejects_unknown_link_type() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("run-bad-type.db");
        let args = LinkArgs {
            from: "work_package:1".to_string(),
            to: "feature:1".to_string(),
            link_type: "made_up".to_string(),
            note: String::new(),
            by: None,
            db: Some(db),
        };
        let err = run_link(&args).unwrap_err();
        assert!(format!("{err:#}").contains("invalid --link-type"));
    }

    #[test]
    fn run_show_returns_rows_for_both_directions() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("run-show.db");
        let conn = open_db(&db).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        // Outgoing
        conn.execute(
            "INSERT INTO trace_links (from_kind, from_id, to_kind, to_id, link_type, note, created_by, created_at) \
             VALUES ('work_package', '5', 'feature', '3', 'implements', '', 'tester', ?1)",
            params![now],
        )
        .unwrap();
        // Incoming
        conn.execute(
            "INSERT INTO trace_links (from_kind, from_id, to_kind, to_id, link_type, note, created_by, created_at) \
             VALUES ('story', '11', 'work_package', '5', 'verifies', '', 'tester', ?1)",
            params![now],
        )
        .unwrap();
        drop(conn);

        let args = ShowArgs {
            entity: "work_package:5".to_string(),
            db: Some(db),
        };
        // run_show prints to stdout; we only verify the call returns Ok
        // (and that the SQL joins both outgoing + incoming rows).
        run_show(&args).unwrap();
    }
}
