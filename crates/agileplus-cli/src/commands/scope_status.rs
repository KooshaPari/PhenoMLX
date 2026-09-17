// SPDX-License-Identifier: MIT OR Apache-2.0
//! `ap scope-status` â€” show the active cycle and the modules within
//! its scope, plus a count of features associated with each module.
//!
//! Scope status is the high-level "where are we working right now?"
//! view: one cycle (the active one) and the modules in that cycle's
//! scope, with their feature counts.
//!
//! If no active cycle exists, the command lists all cycles so the
//! operator can pick one to activate.
//!
//! # Example
//!
//! ```text
//! $ ap scope-status
//! Active cycle: Sprint 1 (2026-05-26 â†’ 2026-06-09) â€” module scope id=1
//! Modules in scope:
//!  ID  SLUG             NAME                  FEATURES
//!   1  core-platform    Core Platform              2
//!   2  persistence      Persistence                1
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use rusqlite::Connection;

#[derive(Debug, Args)]
pub struct ScopeStatusArgs {
    /// Emit JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,

    /// Path to the SQLite database file. Defaults to `./agileplus.db`.
    #[arg(long, default_value = "agileplus.db")]
    pub db: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CycleInfo {
    pub id: i64,
    pub name: String,
    pub state: String,
    pub start_date: String,
    pub end_date: String,
    pub module_scope_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub id: i64,
    pub slug: String,
    pub friendly_name: String,
    pub feature_count: i64,
}

pub fn run(args: &ScopeStatusArgs) -> Result<()> {
    let conn = Connection::open(&args.db)
        .with_context(|| format!("opening db at {}", args.db.display()))?;

    let cycles = load_cycles(&conn)?;
    let active = cycles.iter().find(|c| c.state == "Active");
    let modules = match active.and_then(|c| c.module_scope_id) {
        Some(scope) => load_modules_in_subtree(&conn, scope)?,
        None => load_all_modules(&conn)?,
    };

    if args.json {
        let active_value = active.map(cycle_to_json);
        let cycles_value: Vec<serde_json::Value> = cycles.iter().map(cycle_to_json).collect();
        let modules_value: Vec<serde_json::Value> = modules.iter().map(module_to_json).collect();
        let payload = serde_json::json!({
            "active": active_value,
            "cycles": cycles_value,
            "modules": modules_value,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    match active {
        Some(c) => {
            println!(
                "Active cycle: {} ({} â†’ {}){}",
                c.name,
                c.start_date,
                c.end_date,
                c.module_scope_id
                    .map(|id| format!(" â€” module scope id={id}"))
                    .unwrap_or_default()
            );
        }
        None => {
            println!("No active cycle. All known cycles:");
            for c in &cycles {
                println!(
                    "  [{}] {:<20}  {:<10}  {} â†’ {}",
                    c.id, c.name, c.state, c.start_date, c.end_date
                );
            }
        }
    }

    if modules.is_empty() {
        println!("\nNo modules in scope.");
        return Ok(());
    }

    println!("\nModules:");
    println!("  {:<3}  {:<18}  {:<28}  FEATURES", "ID", "SLUG", "NAME");
    println!("  {}", "-".repeat(60));
    for m in &modules {
        println!(
            "  {:<3}  {:<18}  {:<28}  {}",
            m.id,
            m.slug,
            truncate(&m.friendly_name, 28),
            m.feature_count
        );
    }

    Ok(())
}

fn cycle_to_json(c: &CycleInfo) -> serde_json::Value {
    serde_json::json!({
        "id": c.id,
        "name": c.name,
        "state": c.state,
        "start_date": c.start_date,
        "end_date": c.end_date,
        "module_scope_id": c.module_scope_id,
    })
}

fn module_to_json(m: &ModuleInfo) -> serde_json::Value {
    serde_json::json!({
        "id": m.id,
        "slug": m.slug,
        "friendly_name": m.friendly_name,
        "feature_count": m.feature_count,
    })
}

fn load_cycles(conn: &Connection) -> Result<Vec<CycleInfo>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, state, start_date, end_date, module_scope_id \
             FROM cycles ORDER BY start_date DESC, id DESC",
        )
        .context("preparing cycles query")?;
    let rows = stmt.query_map([], |r| {
        Ok(CycleInfo {
            id: r.get(0)?,
            name: r.get(1)?,
            state: r.get(2)?,
            start_date: r.get(3)?,
            end_date: r.get(4)?,
            module_scope_id: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn load_all_modules(conn: &Connection) -> Result<Vec<ModuleInfo>> {
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.slug, m.friendly_name, \
                    COALESCE((SELECT COUNT(*) FROM module_feature_tags mft WHERE mft.module_id = m.id), 0) \
             FROM modules m ORDER BY m.id",
        )
        .context("preparing modules query")?;
    let rows = stmt.query_map([], |r| {
        Ok(ModuleInfo {
            id: r.get(0)?,
            slug: r.get(1)?,
            friendly_name: r.get(2)?,
            feature_count: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn load_modules_in_subtree(conn: &Connection, root_id: i64) -> Result<Vec<ModuleInfo>> {
    // Walk the `modules.parent_module_id` tree breadth-first and
    // collect every descendant of `root_id` (including root itself).
    let mut ids = vec![root_id];
    let mut frontier = vec![root_id];
    while !frontier.is_empty() {
        let placeholders = std::iter::repeat_n("?", frontier.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT id FROM modules WHERE parent_module_id IN ({placeholders})");
        let mut stmt = conn
            .prepare(&sql)
            .context("preparing modules child query")?;
        let params: Vec<&dyn rusqlite::ToSql> =
            frontier.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(params), |r| r.get::<_, i64>(0))?;
        let mut next = Vec::new();
        for row in rows {
            let id = row?;
            if !ids.contains(&id) {
                ids.push(id);
                next.push(id);
            }
        }
        frontier = next;
    }

    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT m.id, m.slug, m.friendly_name, \
                COALESCE((SELECT COUNT(*) FROM module_feature_tags mft WHERE mft.module_id = m.id), 0) \
         FROM modules m WHERE m.id IN ({placeholders}) ORDER BY m.id"
    );
    let mut stmt = conn
        .prepare(&sql)
        .context("preparing modules scope query")?;
    let params: Vec<&dyn rusqlite::ToSql> = ids.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(rusqlite::params_from_iter(params), |r| {
        Ok(ModuleInfo {
            id: r.get(0)?,
            slug: r.get(1)?,
            friendly_name: r.get(2)?,
            feature_count: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{t}â€¦")
}

#[cfg(test)]
mod tests {
    use super::*;

    use rusqlite::Connection;

    fn schema(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE modules (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 slug TEXT NOT NULL,
                 friendly_name TEXT NOT NULL,
                 description TEXT,
                 parent_module_id INTEGER REFERENCES modules(id) ON DELETE RESTRICT,
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL,
                 UNIQUE (parent_module_id, slug)
             );
             CREATE TABLE features (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 slug TEXT UNIQUE NOT NULL,
                 friendly_name TEXT NOT NULL,
                 state TEXT NOT NULL,
                 spec_hash BLOB NOT NULL,
                 target_branch TEXT NOT NULL DEFAULT 'main',
                 module_id INTEGER REFERENCES modules(id) ON DELETE SET NULL,
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );
             CREATE TABLE module_feature_tags (
                 module_id INTEGER NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
                 feature_id INTEGER NOT NULL REFERENCES features(id) ON DELETE CASCADE,
                 created_at TEXT NOT NULL,
                 PRIMARY KEY (module_id, feature_id)
             );
             CREATE TABLE cycles (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL UNIQUE,
                 description TEXT,
                 state TEXT NOT NULL DEFAULT 'Draft',
                 start_date TEXT NOT NULL,
                 end_date TEXT NOT NULL,
                 module_scope_id INTEGER REFERENCES modules(id) ON DELETE SET NULL,
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );",
        )
        .unwrap();
    }

    fn seed(conn: &Connection) {
        conn.execute(
            "INSERT INTO modules (id, slug, friendly_name, created_at, updated_at) \
             VALUES (1, 'core', 'Core', '2026-05-26T10:00:00', '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO modules (id, slug, friendly_name, parent_module_id, created_at, updated_at) \
             VALUES (2, 'persistence', 'Persistence', 1, '2026-05-26T10:00:00', '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO features (id, slug, friendly_name, state, spec_hash, module_id, created_at, updated_at) \
             VALUES (10, 'f1', 'F1', 'created', X'00', 1, '2026-05-26T10:00:00', '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO module_feature_tags (module_id, feature_id, created_at) \
             VALUES (1, 10, '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO cycles (name, state, start_date, end_date, module_scope_id, created_at, updated_at) \
             VALUES ('Sprint 1', 'Active', '2026-05-26', '2026-06-09', 1, '2026-05-26T10:00:00', '2026-05-26T10:00:00')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn load_cycles_returns_seeded_cycle() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        seed(&conn);
        let cycles = load_cycles(&conn).unwrap();
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].name, "Sprint 1");
        assert_eq!(cycles[0].state, "Active");
        assert_eq!(cycles[0].module_scope_id, Some(1));
    }

    #[test]
    fn load_modules_in_subtree_includes_children() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        seed(&conn);
        let modules = load_modules_in_subtree(&conn, 1).unwrap();
        let ids: Vec<i64> = modules.iter().map(|m| m.id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
    }

    #[test]
    fn load_modules_in_subtree_empty_when_root_missing() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        let modules = load_modules_in_subtree(&conn, 99).unwrap();
        assert!(modules.is_empty());
    }

    #[test]
    fn load_all_modules_returns_all() {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        seed(&conn);
        let modules = load_all_modules(&conn).unwrap();
        assert_eq!(modules.len(), 2);
        let m1 = modules.iter().find(|m| m.id == 1).unwrap();
        assert_eq!(m1.feature_count, 1);
    }

    #[test]
    fn truncate_shortens_long_strings() {
        // The helper caps the result at `max` visible characters:
        // `max - 1` original characters plus the ellipsis.
        assert_eq!(truncate("Persistence", 5), "Persâ€¦");
    }

    #[test]
    fn truncate_keeps_short_strings() {
        assert_eq!(truncate("Core", 10), "Core");
    }
}
