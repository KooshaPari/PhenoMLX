// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ap gate-run` â€” execute the active policy rules in the local store
//! against a single feature + work-package pair, print the result, and
//! return a non-zero exit code if any rule fails.
//!
//! This is a deliberately-simple gate runner: it does not actually
//! interpret each rule's DSL â€” it records the run, prints the active
//! rules, and writes a `metrics` row whose command is `gate-run` and
//! whose `review_cycles` increments by 1 for any failing rule.
//!
//! # Example
//!
//! ```text
//! $ ap gate-run --feature 1 --wp 3
//! Active rules: 5
//!   [1] security  no-root                       PASS
//!   [2] quality    min-coverage-80               PASS
//!   [3] compliance sbom-required                 FAIL  (no SBOM evidence on file)
//! ... gate-run: 1 failure
//! $ echo $?
//! 1
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use rusqlite::Connection;

#[derive(Debug, Args)]
pub struct GateRunArgs {
    /// Feature id the gate is running against.
    #[arg(long, value_name = "ID")]
    pub feature: i64,

    /// Work-package id the gate is running against.
    #[arg(long, value_name = "ID")]
    pub wp: i64,

    /// Optional human-readable note attached to the metrics row.
    #[arg(long, value_name = "TEXT")]
    pub note: Option<String>,

    /// Emit JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,

    /// Path to the SQLite database file. Defaults to `./agileplus.db`.
    #[arg(long, default_value = "agileplus.db")]
    pub db: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GateRule {
    pub id: i64,
    pub domain: String,
    pub rule: String,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct GateRunResult {
    pub feature_id: i64,
    pub wp_id: i64,
    pub rules: Vec<(
        GateRule,
        bool,           /* passed */
        Option<String>, /* reason */
    )>,
    pub failure_count: usize,
}

pub fn run(args: &GateRunArgs) -> Result<()> {
    let conn = Connection::open(&args.db)
        .with_context(|| format!("opening db at {}", args.db.display()))?;

    let result = execute(&conn, args.feature, args.wp)?;

    // Record the run in `metrics` for the dashboard / worklog views.
    record_metrics(
        &conn,
        args.feature,
        result.failure_count as i64,
        result.failure_count as i64,
        args.note.as_deref(),
    )?;

    if args.json {
        let payload = serde_json::json!({
            "feature_id": result.feature_id,
            "wp_id": result.wp_id,
            "failure_count": result.failure_count,
            "rules": result.rules.iter().map(|(r, ok, reason)| {
                serde_json::json!({
                    "id": r.id,
                    "domain": r.domain,
                    "rule": r.rule,
                    "active": r.active,
                    "passed": ok,
                    "reason": reason,
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!(
            "Gate run for feature={} wp={} ({} active rule(s))",
            result.feature_id,
            result.wp_id,
            result.rules.iter().filter(|(r, _, _)| r.active).count()
        );
        for (rule, passed, reason) in &result.rules {
            if !rule.active {
                println!(
                    "  [{}] {:<10}  {:<28}  SKIP  (inactive)",
                    rule.id, rule.domain, rule.rule
                );
                continue;
            }
            let verdict = if *passed { "PASS" } else { "FAIL" };
            let tail = match reason {
                Some(r) => format!("  ({r})"),
                None => String::new(),
            };
            println!(
                "  [{}] {:<10}  {:<28}  {verdict}{tail}",
                rule.id, rule.domain, rule.rule
            );
        }
        if result.failure_count > 0 {
            println!("gate-run: {} failure(s)", result.failure_count);
        } else {
            println!("gate-run: all active rules passed");
        }
    }

    if result.failure_count > 0 {
        // Use a non-zero exit code so CI can detect the failure.
        std::process::exit(1);
    }

    Ok(())
}

/// Execute the gate; shared with the test suite.
pub fn execute(conn: &Connection, feature_id: i64, wp_id: i64) -> Result<GateRunResult> {
    let rules = load_rules(conn)?;
    let mut evaluated = Vec::with_capacity(rules.len());
    let mut failure_count = 0usize;

    for rule in &rules {
        if !rule.active {
            evaluated.push((rule.clone(), false, Some("rule inactive".to_string())));
            continue;
        }
        // Run the actual gate. We do a simple domain-specific check
        // here â€” if the rule's domain is `compliance`, we require at
        // least one evidence row to exist for the work package; for
        // other domains, the rule passes by default.
        let (passed, reason) = if rule.domain == "compliance" {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM evidence WHERE wp_id = ?1",
                    rusqlite::params![wp_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if count > 0 {
                (true, None)
            } else {
                (
                    false,
                    Some("no evidence attached to this work package".to_string()),
                )
            }
        } else {
            (true, None)
        };
        if !passed {
            failure_count += 1;
        }
        evaluated.push((rule.clone(), passed, reason));
    }

    Ok(GateRunResult {
        feature_id,
        wp_id,
        rules: evaluated,
        failure_count,
    })
}

fn load_rules(conn: &Connection) -> Result<Vec<GateRule>> {
    let mut stmt = conn
        .prepare("SELECT id, domain, rule, active FROM policy_rules ORDER BY id")
        .context("preparing policy_rules query")?;
    let rows = stmt.query_map([], |r| {
        Ok(GateRule {
            id: r.get(0)?,
            domain: r.get(1)?,
            rule: r.get(2)?,
            active: r.get::<_, i64>(3)? != 0,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn record_metrics(
    conn: &Connection,
    feature_id: i64,
    agent_runs: i64,
    review_cycles: i64,
    note: Option<&str>,
) -> Result<()> {
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let metadata = note.map(|n| format!("{{\"note\":\"{}\"}}", n.replace('"', "\\\"")));
    conn.execute(
        "INSERT INTO metrics (feature_id, command, duration_ms, agent_runs, review_cycles, metadata, timestamp) \
         VALUES (?1, 'gate-run', 0, ?2, ?3, ?4, ?5)",
        rusqlite::params![feature_id, agent_runs, review_cycles, metadata, timestamp],
    )
    .context("inserting gate-run metrics row")?;
    Ok(())
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
             CREATE TABLE work_packages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 feature_id INTEGER NOT NULL REFERENCES features(id) ON DELETE CASCADE,
                 title TEXT NOT NULL,
                 state TEXT NOT NULL,
                 sequence INTEGER NOT NULL DEFAULT 0,
                 file_scope TEXT NOT NULL DEFAULT '[]',
                 acceptance_criteria TEXT NOT NULL DEFAULT '',
                 agent_id TEXT,
                 pr_url TEXT,
                 pr_state TEXT,
                 worktree_path TEXT,
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );
             CREATE TABLE evidence (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 wp_id INTEGER NOT NULL REFERENCES work_packages(id) ON DELETE CASCADE,
                 fr_id TEXT NOT NULL,
                 evidence_type TEXT NOT NULL,
                 artifact_path TEXT NOT NULL,
                 metadata TEXT,
                 created_at TEXT NOT NULL
             );
             CREATE TABLE policy_rules (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 domain TEXT NOT NULL,
                 rule TEXT NOT NULL,
                 active INTEGER NOT NULL DEFAULT 1,
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

    fn seed(conn: &Connection) {
        conn.execute(
            "INSERT INTO features (id, slug, friendly_name, state, spec_hash, created_at, updated_at) \
             VALUES (1, 'f1', 'F1', 'created', X'00', '2026-05-26T10:00:00', '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO work_packages (id, feature_id, title, state, created_at, updated_at) \
             VALUES (10, 1, 'WP-1', 'planned', '2026-05-26T10:00:00', '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO policy_rules (domain, rule, active, created_at, updated_at) \
             VALUES ('security', 'no-root', 1, '2026-05-26T10:00:00', '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO policy_rules (domain, rule, active, created_at, updated_at) \
             VALUES ('compliance', 'sbom-required', 1, '2026-05-26T10:00:00', '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO policy_rules (domain, rule, active, created_at, updated_at) \
             VALUES ('quality', 'min-coverage-80', 0, '2026-05-26T10:00:00', '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn execute_with_no_evidence_fails_compliance_rule() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        seed(&conn);
        let result = execute(&conn, 1, 10).unwrap();
        assert_eq!(result.rules.len(), 3);
        // The compliance rule must fail; the security rule must pass.
        let compliance = result
            .rules
            .iter()
            .find(|(r, _, _)| r.domain == "compliance")
            .unwrap();
        assert!(!compliance.1);
        let security = result
            .rules
            .iter()
            .find(|(r, _, _)| r.domain == "security")
            .unwrap();
        assert!(security.1);
        assert_eq!(result.failure_count, 1);
    }

    #[test]
    fn execute_passes_when_evidence_present() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        seed(&conn);
        conn.execute(
            "INSERT INTO evidence (wp_id, fr_id, evidence_type, artifact_path, created_at) \
             VALUES (10, 'FR-1', 'test_result', '/tmp/x', '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
        let result = execute(&conn, 1, 10).unwrap();
        assert_eq!(result.failure_count, 0);
    }

    #[test]
    fn execute_skips_inactive_rules_without_failing() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        seed(&conn);
        let result = execute(&conn, 1, 10).unwrap();
        let inactive = result
            .rules
            .iter()
            .find(|(r, _, _)| r.rule == "min-coverage-80")
            .unwrap();
        assert!(!inactive.0.active);
    }

    #[test]
    fn record_metrics_writes_a_metrics_row() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        seed(&conn);
        record_metrics(&conn, 1, 1, 2, Some("hello")).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM metrics WHERE command = 'gate-run'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
