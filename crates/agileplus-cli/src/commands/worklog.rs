//! agileplus-cli worklog subcommand.
//!
//! Two related features share this command tree:
//!
//! 1. **File-level worklog management** (L2 #25) â€” validate, convert,
//!    print, and list worklog JSON files against the canonical 8-field
//!    schema in `WORKLOG_SCHEMA_2026_06_10.md`. Subcommands: `validate`,
//!    `convert`, `schema`, `list`.
//!
//! 2. **Database-backed ingest / query** (L2 #39) â€” read worklog JSON
//!    files, validate them, and insert a row into the `worklog_entries`
//!    SQLite table; query that table back as a table or JSON. Subcommands:
//!    `emit`, `show`.
//!
//! The schema validated by both surfaces is the same canonical schema
//! (`status`, `task_id`, `agent_id`, `files_changed`, `commit_sha`,
//! `verification_result`, `started_at`, `completed_at`).

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

// â”€â”€ Schema constants (L1 WORKLOG_SCHEMA_2026_06_10.md) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

const CANONICAL_FIELDS: &[&str] = &[
    "status",
    "task_id",
    "agent_id",
    "files_changed",
    "commit_sha",
    "verification_result",
    "started_at",
    "completed_at",
];

const CANONICAL_STATUSES: &[&str] = &[
    "pending",
    "running",
    "blocked",
    "completed",
    "failed",
    "cancelled",
];
const CANONICAL_VERIFICATION_STATUSES: &[&str] = &["passed", "failed", "not_run", "partial"];

// â”€â”€ File-level types (L2 #25) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Debug, Serialize, Deserialize)]
struct CanonicalWorklog {
    status: String,
    task_id: String,
    agent_id: String,
    files_changed: Vec<String>,
    commit_sha: String,
    verification_result: VerificationResult,
    started_at: Option<String>,
    completed_at: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    #[serde(default)]
    status: String,
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    notes: String,
}

// â”€â”€ CLI surface â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Top-level worklog command.
///
/// The optional `dir` is honored only by the file-level subcommands
/// (`validate`/`convert`/`schema`/`list`). The database-backed subcommands
/// (`emit`/`show`) source their input from explicit `--from` and filter
/// flags.
#[derive(Debug, Args)]
pub struct WorklogArgs {
    /// Working directory (defaults to current directory). Used by the
    /// file-level subcommands only.
    #[arg(long, short = 'd', default_value = ".")]
    pub dir: PathBuf,

    #[command(subcommand)]
    pub action: WorklogAction,
}

#[derive(Debug, Subcommand)]
pub enum WorklogAction {
    /// Validate worklog JSON files in DIR against the canonical schema.
    Validate,
    /// Convert worklog JSON files to canonical schema. Writes
    /// `worklog-*-canonical.json` next to each source file unless
    /// `--in-place` is passed.
    Convert {
        /// Replace original files with converted content.
        #[arg(long)]
        in_place: bool,
    },
    /// Print the canonical 8-field schema.
    Schema,
    /// List all worklog files (raw + canonical) in DIR.
    List,
    /// Read one or more worklog JSON files and insert them into the
    /// `worklog_entries` table (L2 #39).
    Emit(EmitArgs),
    /// Query `worklog_entries` and print a table or JSON (L2 #39).
    Show(ShowArgs),
}

#[derive(Debug, Args)]
pub struct EmitArgs {
    /// Path to a single worklog JSON file or a directory of worklog JSON
    /// files. When a directory is given, every `*.json` file directly
    /// under it is loaded (non-recursive).
    #[arg(long, value_name = "PATH")]
    pub from: PathBuf,

    /// Print a detailed per-file report.
    #[arg(long)]
    pub verbose: bool,

    /// Force re-ingest by removing any existing row with the same
    /// `(task_id, source_path)` before inserting.
    #[arg(long)]
    pub replace: bool,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Filter by `task_id` (exact match).
    #[arg(long)]
    pub task: Option<String>,

    /// Filter by `status` (exact match).
    #[arg(long)]
    pub status: Option<String>,

    /// Cap the number of rows printed.
    #[arg(long, default_value_t = 50)]
    pub limit: i64,

    /// Emit JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,
}

// â”€â”€ Database-backed types (L2 #39) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Top-level worklog payload (validation target).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorklogPayload {
    pub status: String,
    pub task_id: String,
    pub agent_id: String,
    #[serde(default)]
    pub files_changed: Vec<String>,
    pub commit_sha: Option<String>,
    pub verification_result: VerificationResult,
    pub started_at: String,
    pub completed_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Row representation after read-back from the `worklog_entries` table.
#[derive(Debug, Clone)]
pub struct WorklogEntry {
    pub id: i64,
    pub status: String,
    pub task_id: String,
    pub agent_id: String,
    pub files_changed: Vec<String>,
    pub commit_sha: Option<String>,
    pub verification: VerificationResult,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub source_path: String,
    pub ingested_at: String,
}

// â”€â”€ Public dispatch â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Top-level entry point dispatched from `main.rs`. Resolves the database
/// path internally from the `AGILEPLUS_DB` environment variable for the
/// database-backed subcommands.
pub fn run(args: &WorklogArgs) -> Result<()> {
    match &args.action {
        WorklogAction::Schema => {
            println!("Canonical worklog schema (8 fields):");
            for (i, f) in CANONICAL_FIELDS.iter().enumerate() {
                println!("  {}. {}", i + 1, f);
            }
            Ok(())
        }
        WorklogAction::List => list(&args.dir),
        WorklogAction::Validate => validate(&args.dir),
        WorklogAction::Convert { in_place } => convert(&args.dir, *in_place),
        WorklogAction::Emit(e) => {
            let db_path = db_path_from_env();
            let report = run_emit(e, &db_path)?;
            println!(
                "Emit complete: {} file(s) loaded, {} row(s) inserted, {} replaced, {} skipped.",
                report.files_loaded,
                report.rows_inserted,
                report.rows_replaced,
                report.files_skipped
            );
            if !report.validation_errors.is_empty() {
                eprintln!("Validation / IO errors:");
                for (path, msg) in &report.validation_errors {
                    eprintln!("  {}: {}", path.display(), msg);
                }
            }
            Ok(())
        }
        WorklogAction::Show(s) => {
            let db_path = db_path_from_env();
            let entries = run_show(s, &db_path)?;
            if s.json {
                let projection: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "id": e.id,
                            "task_id": e.task_id,
                            "status": e.status,
                            "agent_id": e.agent_id,
                            "files_changed": e.files_changed,
                            "commit_sha": e.commit_sha,
                            "verification": {
                                "status": e.verification.status,
                                "notes": e.verification.notes,
                            },
                            "started_at": e.started_at,
                            "completed_at": e.completed_at,
                            "source_path": e.source_path,
                            "ingested_at": e.ingested_at,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&projection)?);
            } else {
                print_table(&entries);
            }
            Ok(())
        }
    }
}

/// Top-level entry point with an explicit database path. Preferred when the
/// caller (e.g. `main.rs`) has already resolved the path via
/// `db_path_from_env()` so the env-var lookup happens exactly once.
pub fn run_with_db(args: &WorklogArgs, db_path: &Path) -> Result<()> {
    match &args.action {
        WorklogAction::Schema => {
            println!("Canonical worklog schema (8 fields):");
            for (i, f) in CANONICAL_FIELDS.iter().enumerate() {
                println!("  {}. {}", i + 1, f);
            }
            Ok(())
        }
        WorklogAction::List => list(&args.dir),
        WorklogAction::Validate => validate(&args.dir),
        WorklogAction::Convert { in_place } => convert(&args.dir, *in_place),
        WorklogAction::Emit(e) => {
            let report = run_emit(e, db_path)?;
            println!(
                "Emit complete: {} file(s) loaded, {} row(s) inserted, {} replaced, {} skipped.",
                report.files_loaded,
                report.rows_inserted,
                report.rows_replaced,
                report.files_skipped
            );
            if !report.validation_errors.is_empty() {
                eprintln!("Validation / IO errors:");
                for (path, msg) in &report.validation_errors {
                    eprintln!("  {}: {}", path.display(), msg);
                }
            }
            Ok(())
        }
        WorklogAction::Show(s) => {
            let entries = run_show(s, db_path)?;
            if s.json {
                let projection: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "id": e.id,
                            "task_id": e.task_id,
                            "status": e.status,
                            "agent_id": e.agent_id,
                            "files_changed": e.files_changed,
                            "commit_sha": e.commit_sha,
                            "verification": {
                                "status": e.verification.status,
                                "notes": e.verification.notes,
                            },
                            "started_at": e.started_at,
                            "completed_at": e.completed_at,
                            "source_path": e.source_path,
                            "ingested_at": e.ingested_at,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&projection)?);
            } else {
                print_table(&entries);
            }
            Ok(())
        }
    }
}

pub(crate) fn db_path_from_env() -> PathBuf {
    std::env::var("AGILEPLUS_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("agileplus.db"))
}

// â”€â”€ File-level commands (L2 #25) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

fn list(dir: &Path) -> Result<()> {
    let raw = find_worklogs(dir, false)?;
    let canonical = find_worklogs(dir, true)?;
    println!("Raw worklogs in {}:", dir.display());
    for p in raw {
        println!("  {}", p.display());
    }
    println!("\nCanonical worklogs:");
    for p in canonical {
        println!("  {}", p.display());
    }
    Ok(())
}

fn validate(dir: &Path) -> Result<()> {
    let files = find_worklogs(dir, false)?;
    println!(
        "Validating {} worklog(s) in {} against canonical schema...",
        files.len(),
        dir.display()
    );
    let mut ok = 0;
    let mut err = 0;
    for path in &files {
        match std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            Some(v) => {
                if let Some(obj) = v.as_object() {
                    let missing: Vec<&&str> = CANONICAL_FIELDS
                        .iter()
                        .filter(|f| !obj.contains_key(**f))
                        .collect();
                    if missing.is_empty() {
                        println!("OK   {}", path.display());
                        ok += 1;
                    } else {
                        println!(
                            "FAIL {}: missing {}",
                            path.display(),
                            missing.iter().map(|s| **s).collect::<Vec<_>>().join(", ")
                        );
                        err += 1;
                    }
                } else {
                    println!("FAIL {}: not a JSON object", path.display());
                    err += 1;
                }
            }
            None => {
                println!("FAIL {}: invalid JSON", path.display());
                err += 1;
            }
        }
    }
    println!("\nResult: {ok} OK, {err} FAIL");
    if err > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn convert(dir: &Path, in_place: bool) -> Result<()> {
    let files = find_worklogs(dir, false)?;
    println!(
        "Converting {} worklog(s) in {} (in_place={})",
        files.len(),
        dir.display(),
        in_place
    );
    for path in &files {
        let content = std::fs::read_to_string(path)?;
        let raw: serde_json::Value = serde_json::from_str(&content)?;
        let canonical = to_canonical(&raw);
        let pretty = serde_json::to_string_pretty(&canonical)?;
        let target = if in_place {
            path.clone()
        } else {
            path_with_suffix(path, "-canonical.json")
        };
        std::fs::write(&target, pretty + "\n")?;
        println!("OK   {} -> {}", path.display(), target.display());
    }
    Ok(())
}

fn to_canonical(raw: &serde_json::Value) -> CanonicalWorklog {
    let obj = raw.as_object();
    let get_str = |k: &str| {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };
    let get_arr = |k: &str| {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    };
    let verification = obj
        .and_then(|o| {
            o.get("verification_result")
                .cloned()
                .or_else(|| o.get("verification").cloned())
        })
        .map(|v| {
            let status = v
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("not_run")
                .to_string();
            let commands = v
                .get("commands")
                .and_then(|c| c.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let notes = v
                .get("notes")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            VerificationResult {
                status,
                commands,
                notes,
            }
        })
        .unwrap_or_default();

    CanonicalWorklog {
        status: get_str("status").unwrap_or_else(|| "completed".to_string()),
        task_id: get_str("task_id")
            .or_else(|| get_str("task"))
            .unwrap_or_else(|| "unknown".to_string()),
        agent_id: get_str("agent_id").unwrap_or_else(|| "codex-exec".to_string()),
        files_changed: {
            let v: Vec<String> = get_arr("files_changed");
            if v.is_empty() {
                get_arr("files")
            } else {
                v
            }
        },
        commit_sha: get_str("commit_sha")
            .or_else(|| get_str("branch"))
            .or_else(|| get_str("merge_commit"))
            .unwrap_or_else(|| "unknown".to_string()),
        verification_result: verification,
        started_at: get_str("started_at"),
        completed_at: get_str("completed_at").or_else(|| get_str("date")),
    }
}

fn find_worklogs(dir: &Path, canonical_only: bool) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let is_worklog = name_str.starts_with("worklog-") && name_str.ends_with(".json");
        let is_canonical = name_str.contains("-canonical.json");
        if is_worklog && (canonical_only == is_canonical || (!canonical_only && is_canonical)) {
            out.push(entry.path());
        }
    }
    out.sort();
    Ok(out)
}

fn path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_suffix(".json") {
        PathBuf::from(format!("{stripped}{suffix}"))
    } else {
        path.with_extension(suffix)
    }
}

// â”€â”€ Database-backed validation (L2 #39) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Validate a parsed [`WorklogPayload`] against the canonical schema.
pub fn validate_payload(payload: &WorklogPayload) -> Result<()> {
    if !CANONICAL_STATUSES.contains(&payload.status.as_str()) {
        bail!(
            "invalid status '{}': expected one of {}",
            payload.status,
            CANONICAL_STATUSES.join(", ")
        );
    }
    if payload.task_id.trim().is_empty() {
        bail!("task_id must be a non-empty string");
    }
    if payload.agent_id.trim().is_empty() {
        bail!("agent_id must be a non-empty string");
    }
    if let Some(ref sha) = payload.commit_sha {
        if !is_valid_sha(sha) {
            bail!("commit_sha '{sha}' is not a 7-40 char hex string and is not null");
        }
    }
    if !CANONICAL_VERIFICATION_STATUSES.contains(&payload.verification_result.status.as_str()) {
        bail!(
            "verification_result.status '{}' is invalid: expected one of {}",
            payload.verification_result.status,
            CANONICAL_VERIFICATION_STATUSES.join(", ")
        );
    }
    if payload.verification_result.status == "not_run"
        && !payload.verification_result.commands.is_empty()
    {
        bail!("verification_result.commands must be empty when status is 'not_run'");
    }
    for cmd in &payload.verification_result.commands {
        if cmd.trim().is_empty() {
            bail!("verification_result.commands contains an empty entry");
        }
    }
    if !is_iso8601_like(&payload.started_at) {
        bail!(
            "started_at '{s}' is not an ISO-8601-like string",
            s = payload.started_at
        );
    }
    if let Some(ref c) = payload.completed_at {
        if !is_iso8601_like(c) {
            bail!("completed_at '{c}' is not an ISO-8601-like string");
        }
    }
    // files_changed: unique, non-empty
    let mut seen = std::collections::HashSet::new();
    for f in &payload.files_changed {
        if f.trim().is_empty() {
            bail!("files_changed contains an empty entry");
        }
        if !seen.insert(f.as_str()) {
            bail!("files_changed contains duplicate entry '{f}'");
        }
    }
    Ok(())
}

fn is_valid_sha(s: &str) -> bool {
    let len = s.len();
    (7..=40).contains(&len)
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// Lenient ISO-8601 check. We only require a `YYYY-MM-DD` prefix followed
/// by some non-empty suffix so we accept `2026-06-10`, `2026-06-10T00:00:00Z`,
/// and `2026-06-10T00:00:00.000+00:00` alike. Full RFC 3339 parsing is left
/// to downstream consumers.
fn is_iso8601_like(s: &str) -> bool {
    if s.len() < 10 {
        return false;
    }
    let bytes = s.as_bytes();
    bytes[0..4].iter().all(|b| b.is_ascii_digit())
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(|b| b.is_ascii_digit())
        && s[10..].chars().any(|c| c != ' ')
}

// â”€â”€ Connection management (L2 #39) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Open the worklog database, ensuring the schema is present.
///
/// If the database does not yet exist, the table is created by running the
/// bundled migration `022_create_worklog_entries.sql` via the regular
/// `MigrationRunner` so it stays in sync with the rest of the schema.
pub fn open_db(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("opening database at {}", db_path.display()))?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .context("enabling foreign keys")?;

    // Run all migrations (this is idempotent â€” applied migrations are
    // skipped). Migration 022 is the one that creates `worklog_entries`.
    let runner = agileplus_sqlite::migrations::MigrationRunner::new(&conn);
    runner
        .run_all()
        .context("applying agileplus migrations (worklog bootstrap)")?;

    Ok(conn)
}

// â”€â”€ Emit logic (L2 #39) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Outcome of a single `emit --from` invocation.
#[derive(Debug, Default)]
pub struct EmitReport {
    pub files_seen: usize,
    pub files_loaded: usize,
    pub files_skipped: usize,
    pub rows_inserted: usize,
    pub rows_replaced: usize,
    pub validation_errors: Vec<(PathBuf, String)>,
}

/// Walk a path: if it is a file load it, if it is a directory load every
/// `*.json` file directly under it (non-recursive). The detailed outcome is
/// returned in an [`EmitReport`].
pub fn run_emit(args: &EmitArgs, db_path: &Path) -> Result<EmitReport> {
    let mut report = EmitReport::default();
    let conn = open_db(db_path)?;
    let files = collect_worklog_files(&args.from, &mut report)?;
    report.files_seen = files.len();

    for path in files {
        let display = path.display().to_string();
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                report.files_skipped += 1;
                report
                    .validation_errors
                    .push((path.clone(), format!("read error: {e}")));
                continue;
            }
        };
        let payload: WorklogPayload = match serde_json::from_str(&raw) {
            Ok(p) => p,
            Err(e) => {
                report.files_skipped += 1;
                report
                    .validation_errors
                    .push((path.clone(), format!("json parse error: {e}")));
                continue;
            }
        };
        if let Err(e) = validate_payload(&payload) {
            report.files_skipped += 1;
            report
                .validation_errors
                .push((path.clone(), format!("{e:#}")));
            continue;
        }

        let inserted = insert_entry(&conn, &payload, &display, &raw, args.replace)?;
        if inserted {
            report.rows_inserted += 1;
        } else if args.replace {
            report.rows_replaced += 1;
        }
        report.files_loaded += 1;

        if args.verbose {
            println!(
                "loaded  task_id={:<10} status={:<10} from {}",
                payload.task_id, payload.status, display
            );
        }
    }

    Ok(report)
}

fn collect_worklog_files(from: &Path, report: &mut EmitReport) -> Result<Vec<PathBuf>> {
    if !from.exists() {
        bail!("--from path does not exist: {}", from.display());
    }
    if from.is_file() {
        return Ok(vec![from.to_path_buf()]);
    }
    let mut out = Vec::new();
    let entries =
        std::fs::read_dir(from).with_context(|| format!("reading directory {}", from.display()))?;
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                report
                    .validation_errors
                    .push((from.to_path_buf(), format!("dir entry error: {e}")));
                continue;
            }
        };
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("json") {
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}

/// Insert a single validated worklog entry. Returns `true` on a new insert,
/// `false` if the row was already present and `--replace` was not set.
pub fn insert_entry(
    conn: &Connection,
    payload: &WorklogPayload,
    source_path: &str,
    raw_payload: &str,
    replace: bool,
) -> Result<bool> {
    let files_changed_json =
        serde_json::to_string(&payload.files_changed).context("serializing files_changed")?;
    let verification_json =
        serde_json::to_string(&payload.verification_result).context("serializing verification")?;
    let ingested_at = chrono::Utc::now().to_rfc3339();

    if replace {
        conn.execute(
            "DELETE FROM worklog_entries WHERE task_id = ?1 AND source_path = ?2",
            rusqlite::params![payload.task_id, source_path],
        )
        .context("deleting prior worklog row")?;
    }

    let rows = conn
        .execute(
            "INSERT OR IGNORE INTO worklog_entries
            (status, task_id, agent_id, files_changed_json, commit_sha,
             verification_json, started_at, completed_at, source_path,
             payload_json, ingested_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            rusqlite::params![
                payload.status,
                payload.task_id,
                payload.agent_id,
                files_changed_json,
                payload.commit_sha,
                verification_json,
                payload.started_at,
                payload.completed_at,
                source_path,
                raw_payload,
                ingested_at,
            ],
        )
        .context("inserting worklog row")?;
    Ok(rows == 1)
}

// â”€â”€ Show logic (L2 #39) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

/// Build a SELECT statement for `show` based on the optional filters. We
/// build the SQL programmatically so unused filters do not pollute the plan.
pub fn run_show(args: &ShowArgs, db_path: &Path) -> Result<Vec<WorklogEntry>> {
    if args.limit <= 0 {
        bail!("--limit must be a positive integer");
    }
    let conn = open_db(db_path)?;

    let mut sql = String::from(
        "SELECT id, status, task_id, agent_id, files_changed_json, commit_sha, \
         verification_json, started_at, completed_at, source_path, ingested_at \
         FROM worklog_entries",
    );
    let mut clauses: Vec<String> = Vec::new();
    if args.task.is_some() {
        clauses.push("task_id = ?".to_string());
    }
    if args.status.is_some() {
        clauses.push("status = ?".to_string());
    }
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY id DESC LIMIT ?");

    let mut stmt = conn.prepare(&sql).context("preparing SELECT statement")?;
    let mut params_dyn: Vec<&dyn rusqlite::ToSql> = Vec::new();
    let task_ref;
    let status_ref;
    if let Some(ref t) = args.task {
        task_ref = t.as_str();
        params_dyn.push(&task_ref);
    }
    if let Some(ref s) = args.status {
        status_ref = s.as_str();
        params_dyn.push(&status_ref);
    }
    let limit_ref = args.limit;
    params_dyn.push(&limit_ref);

    let rows = stmt
        .query_map(params_dyn.as_slice(), row_to_entry)
        .context("executing worklog SELECT")?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.context("reading worklog row")?);
    }
    Ok(entries)
}

fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorklogEntry> {
    let id: i64 = row.get(0)?;
    let status: String = row.get(1)?;
    let task_id: String = row.get(2)?;
    let agent_id: String = row.get(3)?;
    let files_changed_json: String = row.get(4)?;
    let commit_sha: Option<String> = row.get(5)?;
    let verification_json: String = row.get(6)?;
    let started_at: String = row.get(7)?;
    let completed_at: Option<String> = row.get(8)?;
    let source_path: String = row.get(9)?;
    let ingested_at: String = row.get(10)?;

    let files_changed: Vec<String> = serde_json::from_str(&files_changed_json).unwrap_or_default();
    let verification: VerificationResult =
        serde_json::from_str(&verification_json).unwrap_or(VerificationResult {
            status: "not_run".into(),
            commands: vec![],
            notes: String::new(),
        });

    Ok(WorklogEntry {
        id,
        status,
        task_id,
        agent_id,
        files_changed,
        commit_sha,
        verification,
        started_at,
        completed_at,
        source_path,
        ingested_at,
    })
}

/// Render an entry list to a human-readable table. Used by the `show`
/// subcommand's default output mode.
#[allow(clippy::print_literal)] // table header rows use literal strings for column names
pub fn print_table(entries: &[WorklogEntry]) {
    if entries.is_empty() {
        println!("No worklog entries found.");
        return;
    }
    println!(
        "{:<5} {:<10} {:<12} {:<22} {:<24} {}",
        "ID", "TASK", "STATUS", "AGENT", "STARTED", "COMMIT"
    );
    println!("{}", "-".repeat(90));
    for e in entries {
        println!(
            "{:<5} {:<10} {:<12} {:<22} {:<24} {}",
            e.id,
            truncate(&e.task_id, 10),
            truncate(&e.status, 12),
            truncate(&e.agent_id, 22),
            truncate(&e.started_at, 24),
            e.commit_sha.as_deref().unwrap_or("â€”"),
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{t}â€¦")
}

// â”€â”€ Tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn good_payload() -> WorklogPayload {
        WorklogPayload {
            status: "completed".into(),
            task_id: "L2-39".into(),
            agent_id: "forge-l2-39".into(),
            files_changed: vec!["a.rs".into(), "b.rs".into()],
            commit_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
            verification_result: VerificationResult {
                status: "passed".into(),
                commands: vec!["cargo test".into()],
                notes: "ok".into(),
            },
            started_at: "2026-06-11T00:00:00Z".into(),
            completed_at: Some("2026-06-11T00:10:00Z".into()),
            extra: Default::default(),
        }
    }

    fn temp_db_path(tag: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "{tag}-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        path
    }

    fn tempdir_root(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        p
    }

    // â”€â”€ Validation tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    #[test]
    fn validate_payload_accepts_canonical_payload() {
        assert!(validate_payload(&good_payload()).is_ok());
    }

    #[test]
    fn validate_payload_rejects_unknown_status() {
        let mut p = good_payload();
        p.status = "bogus".into();
        let err = validate_payload(&p).unwrap_err().to_string();
        assert!(err.contains("invalid status"), "got: {err}");
    }

    #[test]
    fn validate_payload_rejects_blank_task_id() {
        let mut p = good_payload();
        p.task_id = "   ".into();
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_rejects_blank_agent_id() {
        let mut p = good_payload();
        p.agent_id = String::new();
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_rejects_uppercase_sha() {
        let mut p = good_payload();
        p.commit_sha = Some("ABCDEF1234".into());
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_accepts_short_sha() {
        let mut p = good_payload();
        p.commit_sha = Some("0123456".into());
        assert!(validate_payload(&p).is_ok());
    }

    #[test]
    fn validate_payload_rejects_bad_verification_status() {
        let mut p = good_payload();
        p.verification_result.status = "yolo".into();
        let err = validate_payload(&p).unwrap_err().to_string();
        assert!(err.contains("verification_result.status"), "got: {err}");
    }

    #[test]
    fn validate_payload_rejects_not_run_with_commands() {
        let mut p = good_payload();
        p.verification_result.status = "not_run".into();
        p.verification_result.commands = vec!["cargo test".into()];
        let err = validate_payload(&p).unwrap_err().to_string();
        assert!(err.contains("not_run"), "got: {err}");
    }

    #[test]
    fn validate_payload_rejects_duplicate_files_changed() {
        let mut p = good_payload();
        p.files_changed = vec!["a.rs".into(), "a.rs".into()];
        let err = validate_payload(&p).unwrap_err().to_string();
        assert!(err.contains("duplicate"), "got: {err}");
    }

    #[test]
    fn validate_payload_rejects_empty_command() {
        let mut p = good_payload();
        p.verification_result.commands = vec!["".into()];
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_rejects_bad_iso8601() {
        let mut p = good_payload();
        p.started_at = "yesterday".into();
        assert!(validate_payload(&p).is_err());
    }

    #[test]
    fn validate_payload_rejects_bad_completed_at_iso8601() {
        let mut p = good_payload();
        p.completed_at = Some("2026/06/11 00:00:00".into());
        assert!(validate_payload(&p).is_err());
    }

    // â”€â”€ Database / emit / show tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

    #[test]
    fn open_db_creates_table_on_fresh_path() {
        let db = temp_db_path("agileplus-worklog-open");
        let conn = open_db(&db).expect("open");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM worklog_entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        let _ = std::fs::remove_file(db);
    }

    #[test]
    fn insert_entry_is_idempotent() {
        let db = temp_db_path("agileplus-worklog-idem");
        let conn = open_db(&db).unwrap();
        let p = good_payload();
        let raw = serde_json::to_string(&p).unwrap();
        assert!(insert_entry(&conn, &p, "src/w.json", &raw, false).unwrap());
        // Second insert with same (task_id, source_path) should be a no-op.
        assert!(!insert_entry(&conn, &p, "src/w.json", &raw, false).unwrap());
        // With replace=true, the row should be removed and re-inserted.
        assert!(insert_entry(&conn, &p, "src/w.json", &raw, true).unwrap());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM worklog_entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let _ = std::fs::remove_file(db);
    }

    #[test]
    fn run_emit_inserts_valid_file_and_skips_invalid() {
        let dir = tempdir_root("agileplus-worklog-run-emit");
        std::fs::create_dir_all(&dir).unwrap();
        let good_path = dir.join("good.json");
        let bad_path = dir.join("bad.json");
        let good = serde_json::to_string(&good_payload()).unwrap();
        // Bad: wrong status value.
        let mut bad_map: HashMap<String, serde_json::Value> = HashMap::new();
        bad_map.insert("status".into(), serde_json::json!("bogus"));
        bad_map.insert("task_id".into(), serde_json::json!("L2-39"));
        bad_map.insert("agent_id".into(), serde_json::json!("forge"));
        bad_map.insert("files_changed".into(), serde_json::json!([]));
        bad_map.insert("commit_sha".into(), serde_json::json!(null));
        bad_map.insert(
            "verification_result".into(),
            serde_json::json!({"status":"passed","commands":["x"],"notes":"ok"}),
        );
        bad_map.insert(
            "started_at".into(),
            serde_json::json!("2026-06-11T00:00:00Z"),
        );
        bad_map.insert("completed_at".into(), serde_json::json!(null));
        let bad = serde_json::to_string(&bad_map).unwrap();
        std::fs::write(&good_path, &good).unwrap();
        std::fs::write(&bad_path, &bad).unwrap();

        let db = temp_db_path("agileplus-worklog-run-emit-db");
        let args = EmitArgs {
            from: dir.clone(),
            verbose: false,
            replace: false,
        };
        let report = run_emit(&args, &db).unwrap();
        assert_eq!(report.files_seen, 2);
        assert_eq!(report.files_loaded, 1);
        assert_eq!(report.files_skipped, 1);
        assert_eq!(report.rows_inserted, 1);
        assert_eq!(report.validation_errors.len(), 1);

        let _ = std::fs::remove_file(&db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_show_filters_by_task_and_status() {
        let db = temp_db_path("agileplus-worklog-show-filter");
        let conn = open_db(&db).unwrap();
        let p1 = good_payload();
        let mut p2 = good_payload();
        p2.task_id = "L2-99".into();
        p2.status = "running".into();
        let raw1 = serde_json::to_string(&p1).unwrap();
        let raw2 = serde_json::to_string(&p2).unwrap();
        insert_entry(&conn, &p1, "a.json", &raw1, false).unwrap();
        insert_entry(&conn, &p2, "b.json", &raw2, false).unwrap();

        let all = run_show(
            &ShowArgs {
                task: None,
                status: None,
                limit: 10,
                json: false,
            },
            &db,
        )
        .unwrap();
        assert_eq!(all.len(), 2);

        let only_l239 = run_show(
            &ShowArgs {
                task: Some("L2-39".into()),
                status: None,
                limit: 10,
                json: false,
            },
            &db,
        )
        .unwrap();
        assert_eq!(only_l239.len(), 1);
        assert_eq!(only_l239[0].task_id, "L2-39");

        let only_running = run_show(
            &ShowArgs {
                task: None,
                status: Some("running".into()),
                limit: 10,
                json: false,
            },
            &db,
        )
        .unwrap();
        assert_eq!(only_running.len(), 1);
        assert_eq!(only_running[0].status, "running");

        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn row_to_entry_round_trips_files_changed() {
        let db = temp_db_path("agileplus-worklog-roundtrip");
        let conn = open_db(&db).unwrap();
        let p = good_payload();
        let raw = serde_json::to_string(&p).unwrap();
        insert_entry(&conn, &p, "x.json", &raw, false).unwrap();
        let entries = run_show(
            &ShowArgs {
                task: None,
                status: None,
                limit: 10,
                json: false,
            },
            &db,
        )
        .unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.task_id, "L2-39");
        assert_eq!(e.status, "completed");
        assert_eq!(e.files_changed, vec!["a.rs", "b.rs"]);
        assert_eq!(e.verification.status, "passed");
        assert_eq!(e.verification.commands, vec!["cargo test"]);
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn run_show_rejects_negative_limit() {
        let args = ShowArgs {
            task: None,
            status: None,
            limit: -1,
            json: false,
        };
        let db = temp_db_path("agileplus-worklog-show");
        let res = run_show(&args, &db);
        assert!(res.is_err());
        let _ = std::fs::remove_file(db);
    }
}
