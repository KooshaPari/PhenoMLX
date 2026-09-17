// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ap run-record` — append a `metrics` row recording a CLI run.
//!
//! This is the low-level primitive behind the `gate-run` subcommand
//! and the agent loops. It lets a shell wrapper record a run without
//! needing to drive rusqlite directly.
//!
//! # Example
//!
//! ```text
//! $ ap run-record --feature 1 --command implement --duration-ms 4521 --agent-runs 1
//! Recorded run id=17: feature=1, command=implement, duration=4521ms.
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use rusqlite::Connection;

#[derive(Debug, Args)]
pub struct RunRecordArgs {
    /// Feature id the run was performed against (omit for unscoped runs).
    #[arg(long, value_name = "ID")]
    pub feature: Option<i64>,

    /// Command label recorded in the metrics row (e.g. `implement`,
    /// `review`, `validate`).
    #[arg(long, value_name = "CMD")]
    pub command: String,

    /// Wall-clock duration in milliseconds.
    #[arg(long, default_value_t = 0)]
    pub duration_ms: i64,

    /// Number of agent invocations this run made.
    #[arg(long, default_value_t = 0)]
    pub agent_runs: i64,

    /// Number of review cycles the run went through.
    #[arg(long, default_value_t = 0)]
    pub review_cycles: i64,

    /// Optional metadata blob (JSON). Stored as-is in the metadata column.
    #[arg(long, value_name = "JSON")]
    pub metadata: Option<String>,

    /// Emit JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,

    /// Path to the SQLite database file. Defaults to `./agileplus.db`.
    #[arg(long, default_value = "agileplus.db")]
    pub db: PathBuf,
}

pub fn run(args: &RunRecordArgs) -> Result<()> {
    let command = args.command.trim();
    if command.is_empty() {
        anyhow::bail!("--command must be non-empty");
    }

    let conn = Connection::open(&args.db)
        .with_context(|| format!("opening db at {}", args.db.display()))?;

    let id = insert_run(
        &conn,
        args.feature,
        command,
        args.duration_ms,
        args.agent_runs,
        args.review_cycles,
        args.metadata.as_deref(),
    )?;

    if args.json {
        let payload = serde_json::json!({
            "id": id,
            "feature_id": args.feature,
            "command": command,
            "duration_ms": args.duration_ms,
            "agent_runs": args.agent_runs,
            "review_cycles": args.review_cycles,
            "metadata": args.metadata,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        let feat = args
            .feature
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string());
        println!(
            "Recorded run id={id}: feature={feat}, command={command}, duration={}ms, agent_runs={}, review_cycles={}.",
            args.duration_ms, args.agent_runs, args.review_cycles
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn insert_run(
    conn: &Connection,
    feature: Option<i64>,
    command: &str,
    duration_ms: i64,
    agent_runs: i64,
    review_cycles: i64,
    metadata: Option<&str>,
) -> Result<i64> {
    if duration_ms < 0 || agent_runs < 0 || review_cycles < 0 {
        anyhow::bail!("--duration-ms, --agent-runs and --review-cycles must be non-negative");
    }
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO metrics (feature_id, command, duration_ms, agent_runs, review_cycles, metadata, timestamp) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            feature,
            command,
            duration_ms,
            agent_runs,
            review_cycles,
            metadata,
            timestamp
        ],
    )
    .context("inserting metrics row")?;
    Ok(conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;

    use rusqlite::Connection;

    fn schema(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE features (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 slug TEXT UNIQUE NOT NULL,
                 friendly_name TEXT NOT NULL,
                 state TEXT NOT NULL,
                 spec_hash BLOB NOT NULL,
                 target_branch TEXT NOT NULL DEFAULT 'main',
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );
             CREATE TABLE metrics (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 feature_id INTEGER REFERENCES features(id) ON DELETE SET NULL,
                 command TEXT NOT NULL,
                 duration_ms INTEGER NOT NULL DEFAULT 0,
                 agent_runs INTEGER NOT NULL DEFAULT 0,
                 review_cycles INTEGER NOT NULL DEFAULT 0,
                 metadata TEXT,
                 timestamp TEXT NOT NULL
             );",
        )
        .unwrap();
    }

    #[test]
    fn insert_run_with_feature() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        conn.execute(
            "INSERT INTO features (id, slug, friendly_name, state, spec_hash, created_at, updated_at) \
             VALUES (1, 'f1', 'F1', 'created', X'00', '2026-05-26T10:00:00', '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
        let id = insert_run(&conn, Some(1), "implement", 1234, 1, 0, None).unwrap();
        assert!(id > 0);
        let (feat, cmd, dur, ag, rev, meta): (Option<i64>, String, i64, i64, i64, Option<String>) =
            conn.query_row(
                "SELECT feature_id, command, duration_ms, agent_runs, review_cycles, metadata \
                 FROM metrics WHERE id = ?1",
                rusqlite::params![id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(feat, Some(1));
        assert_eq!(cmd, "implement");
        assert_eq!(dur, 1234);
        assert_eq!(ag, 1);
        assert_eq!(rev, 0);
        assert!(meta.is_none());
    }

    #[test]
    fn insert_run_without_feature_is_allowed() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        let id = insert_run(&conn, None, "plan", 0, 0, 0, Some("{\"foo\":1}")).unwrap();
        assert!(id > 0);
        let feat: Option<i64> = conn
            .query_row(
                "SELECT feature_id FROM metrics WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(feat.is_none());
    }

    #[test]
    fn negative_counters_rejected() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        let result = insert_run(&conn, None, "x", -1, 0, 0, None);
        assert!(result.is_err());
    }
}
