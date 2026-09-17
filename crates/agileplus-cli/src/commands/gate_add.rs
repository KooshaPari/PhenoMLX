// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ap gate-add` — register a new policy rule in the `policy_rules`
//! table. The new rule is active by default; pass `--inactive` to
//! register it in the disabled state.
//!
//! # Example
//!
//! ```text
//! $ ap gate-add --domain security --rule "no-root-shell"
//! Added policy rule [security] no-root-shell (id=4).
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use rusqlite::Connection;

#[derive(Debug, Args)]
pub struct GateAddArgs {
    /// Rule domain — one of: security, quality, compliance, performance, custom.
    #[arg(long, value_name = "DOMAIN")]
    pub domain: String,

    /// Human-readable rule identifier.
    #[arg(long, value_name = "RULE")]
    pub rule: String,

    /// Register the rule as inactive (default is active).
    #[arg(long)]
    pub inactive: bool,

    /// Emit JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,

    /// Path to the SQLite database file. Defaults to `./agileplus.db`.
    #[arg(long, default_value = "agileplus.db")]
    pub db: PathBuf,
}

const ALLOWED_DOMAINS: &[&str] = &["security", "quality", "compliance", "performance", "custom"];

pub fn run(args: &GateAddArgs) -> Result<()> {
    let domain = args.domain.to_lowercase();
    if !ALLOWED_DOMAINS.contains(&domain.as_str()) {
        anyhow::bail!(
            "invalid domain '{}' — must be one of: {}",
            args.domain,
            ALLOWED_DOMAINS.join(", ")
        );
    }
    let rule = args.rule.trim();
    if rule.is_empty() {
        anyhow::bail!("--rule must be non-empty");
    }

    let conn = Connection::open(&args.db)
        .with_context(|| format!("opening db at {}", args.db.display()))?;

    let id = insert_rule(&conn, &domain, rule, !args.inactive)?;

    if args.json {
        let payload = serde_json::json!({
            "id": id,
            "domain": domain,
            "rule": rule,
            "active": !args.inactive,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        let state = if args.inactive { "inactive" } else { "active" };
        println!("Added policy rule [{domain}] {rule} (id={id}, {state}).");
    }

    Ok(())
}

/// Insert a policy rule and return the new id. Exposed for tests.
pub fn insert_rule(conn: &Connection, domain: &str, rule: &str, active: bool) -> Result<i64> {
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO policy_rules (domain, rule, active, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![domain, rule, active as i64, now],
    )
    .context("inserting policy_rule")?;
    Ok(conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;

    use rusqlite::Connection;

    fn schema(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE policy_rules (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 domain TEXT NOT NULL,
                 rule TEXT NOT NULL,
                 active INTEGER NOT NULL DEFAULT 1,
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );",
        )
        .unwrap();
    }

    #[test]
    fn insert_active_rule_returns_id_and_persists() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        let id = insert_rule(&conn, "security", "no-root", true).unwrap();
        assert!(id > 0);
        let (domain, rule, active): (String, String, i64) = conn
            .query_row(
                "SELECT domain, rule, active FROM policy_rules WHERE id = ?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(domain, "security");
        assert_eq!(rule, "no-root");
        assert_eq!(active, 1);
    }

    #[test]
    fn insert_inactive_rule_persists_active_zero() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        let id = insert_rule(&conn, "quality", "min-coverage", false).unwrap();
        let active: i64 = conn
            .query_row(
                "SELECT active FROM policy_rules WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(active, 0);
    }

    #[test]
    fn allowed_domains_contains_expected_values() {
        assert!(ALLOWED_DOMAINS.contains(&"security"));
        assert!(ALLOWED_DOMAINS.contains(&"quality"));
        assert!(ALLOWED_DOMAINS.contains(&"compliance"));
        assert!(ALLOWED_DOMAINS.contains(&"performance"));
        assert!(ALLOWED_DOMAINS.contains(&"custom"));
        assert!(!ALLOWED_DOMAINS.contains(&"bogus"));
    }
}
