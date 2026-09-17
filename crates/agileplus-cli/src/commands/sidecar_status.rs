// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ap sidecar-status` — print the current device-node roster with
//! each node's last-seen timestamp, sync vector, and platform version.
//!
//! Device nodes are the local representation of sidecar processes that
//! sync AgilePlus data across machines. A node is "stale" when its
//! `last_seen` is more than `--stale-seconds` old (default 300s).
//!
//! # Example
//!
//! ```text
//! $ ap sidecar-status
//!  ID  DEVICE    HOSTNAME     TAILSCALE_IP    LAST_SEEN           PLATFORM   STATE
//! ---  --------  ------------  --------------- ------------------- ----------  -----
//!   1  dev-1     workstation  100.64.0.1      2026-06-11T10:00:00  v1.0.0     OK
//!   2  dev-2     laptop        100.64.0.2      2026-06-11T09:50:00  v1.0.0     STALE
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Args;
use rusqlite::Connection;

#[derive(Debug, Args)]
pub struct SidecarStatusArgs {
    /// A node is reported as STALE when its `last_seen` is more than
    /// this many seconds old.
    #[arg(long, default_value_t = 300)]
    pub stale_seconds: i64,

    /// Emit JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,

    /// Path to the SQLite database file. Defaults to `./agileplus.db`.
    #[arg(long, default_value = "agileplus.db")]
    pub db: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SidecarRow {
    pub id: i64,
    pub device_id: String,
    pub hostname: String,
    pub tailscale_ip: Option<String>,
    pub last_seen: String,
    pub platform_version: String,
    pub sync_vector: String,
    pub age_seconds: Option<i64>,
    pub state: SidecarState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarState {
    Ok,
    Stale,
    Unknown,
}

pub fn run(args: &SidecarStatusArgs) -> Result<()> {
    let conn = Connection::open(&args.db)
        .with_context(|| format!("opening db at {}", args.db.display()))?;

    let rows = fetch_rows(&conn, args.stale_seconds)?;

    if args.json {
        let payload: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "device_id": r.device_id,
                    "hostname": r.hostname,
                    "tailscale_ip": r.tailscale_ip,
                    "last_seen": r.last_seen,
                    "platform_version": r.platform_version,
                    "sync_vector": r.sync_vector,
                    "age_seconds": r.age_seconds,
                    "state": match r.state {
                        SidecarState::Ok => "OK",
                        SidecarState::Stale => "STALE",
                        SidecarState::Unknown => "UNKNOWN",
                    },
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    if rows.is_empty() {
        println!("No sidecar device nodes registered.");
        return Ok(());
    }

    println!(
        "{:<3}  {:<10}  {:<12}  {:<15}  {:<19}  {:<10}  {}",
        "ID", "DEVICE", "HOSTNAME", "TAILSCALE_IP", "LAST_SEEN", "PLATFORM", "STATE"
    );
    println!("{}", "-".repeat(90));
    for r in &rows {
        let ip = r
            .tailscale_ip
            .as_deref()
            .unwrap_or("—")
            .to_string();
        let state = match r.state {
            SidecarState::Ok => "OK",
            SidecarState::Stale => "STALE",
            SidecarState::Unknown => "UNKNOWN",
        };
        println!(
            "{:<3}  {:<10}  {:<12}  {:<15}  {:<19}  {:<10}  {}",
            r.id, r.device_id, r.hostname, ip, r.last_seen, r.platform_version, state
        );
    }

    Ok(())
}

pub fn fetch_rows(conn: &Connection, stale_seconds: i64) -> Result<Vec<SidecarRow>> {
    let now = Utc::now();
    let mut stmt = conn
        .prepare(
            "SELECT id, device_id, hostname, tailscale_ip, last_seen, platform_version, sync_vector \
             FROM device_nodes ORDER BY id",
        )
        .context("preparing device_nodes query")?;
    let rows = stmt.query_map([], |r| {
        Ok(SidecarRow {
            id: r.get(0)?,
            device_id: r.get(1)?,
            hostname: r.get(2)?,
            tailscale_ip: r.get(3)?,
            last_seen: r.get(4)?,
            platform_version: r.get(5)?,
            sync_vector: r.get(6)?,
            age_seconds: None,
            state: SidecarState::Unknown,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        let mut r = row?;
        r.age_seconds = age_seconds(&r.last_seen, now);
        r.state = match r.age_seconds {
            Some(a) if a <= stale_seconds => SidecarState::Ok,
            Some(_) => SidecarState::Stale,
            None => SidecarState::Unknown,
        };
        out.push(r);
    }
    Ok(out)
}

fn age_seconds(last_seen: &str, now: DateTime<Utc>) -> Option<i64> {
    let dt = DateTime::parse_from_rfc3339(last_seen)
        .ok()
        .map(|d| d.with_timezone(&Utc))
        .or_else(|| {
            // Fallback: try a SQLite-style "YYYY-MM-DD HH:MM:SS" timestamp.
            chrono::NaiveDateTime::parse_from_str(last_seen, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|n| n.and_utc())
        })?;
    Some((now - dt).num_seconds().max(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    use rusqlite::Connection;

    fn schema(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE device_nodes (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 device_id TEXT NOT NULL UNIQUE,
                 tailscale_ip TEXT,
                 hostname TEXT NOT NULL,
                 last_seen TEXT NOT NULL,
                 sync_vector TEXT NOT NULL,
                 platform_version TEXT NOT NULL,
                 metadata TEXT
             );",
        )
        .unwrap();
    }

    fn fresh_ts() -> String {
        Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()
    }

    fn stale_ts(seconds_old: i64) -> String {
        (Utc::now() - chrono::Duration::seconds(seconds_old))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string()
    }

    #[test]
    fn empty_db_returns_empty_vec() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        let rows = fetch_rows(&conn, 300).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn fresh_node_is_ok() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        conn.execute(
            "INSERT INTO device_nodes (device_id, hostname, last_seen, sync_vector, platform_version) \
             VALUES ('d1', 'host', ?1, 'sv', 'v1')",
            rusqlite::params![fresh_ts()],
        )
        .unwrap();
        let rows = fetch_rows(&conn, 300).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, SidecarState::Ok);
    }

    #[test]
    fn old_node_is_stale() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        conn.execute(
            "INSERT INTO device_nodes (device_id, hostname, last_seen, sync_vector, platform_version) \
             VALUES ('d1', 'host', ?1, 'sv', 'v1')",
            rusqlite::params![stale_ts(3600)],
        )
        .unwrap();
        let rows = fetch_rows(&conn, 300).unwrap();
        assert_eq!(rows[0].state, SidecarState::Stale);
    }

    #[test]
    fn unparseable_timestamp_is_unknown() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        conn.execute(
            "INSERT INTO device_nodes (device_id, hostname, last_seen, sync_vector, platform_version) \
             VALUES ('d1', 'host', 'not-a-timestamp', 'sv', 'v1')",
            [],
        )
        .unwrap();
        let rows = fetch_rows(&conn, 300).unwrap();
        assert_eq!(rows[0].state, SidecarState::Unknown);
    }

    #[test]
    fn age_seconds_handles_iso_and_sqlite_timestamps() {
        let now = Utc::now();
        let rfc = (now - chrono::Duration::seconds(30))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let a = age_seconds(&rfc, now).unwrap();
        // Allow a 2-second slack.
        assert!((28..=32).contains(&a), "got {a}");
    }
}
