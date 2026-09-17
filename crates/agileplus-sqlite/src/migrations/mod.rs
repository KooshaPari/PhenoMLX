// SPDX-License-Identifier: MIT OR Apache-2.0
//! Migration system for agileplus-sqlite.
//!
//! Migrations are embedded as SQL files and applied in order on startup.
//! Applied migrations are tracked in the `_migrations` meta table.

use rusqlite::{Connection, Result as SqlResult};

use agileplus_domain::error::DomainError;

// Embedded SQL migrations
const MIGRATION_001: &str = include_str!("001_create_features.sql");
const MIGRATION_002: &str = include_str!("002_create_work_packages.sql");
const MIGRATION_003: &str = include_str!("003_create_governance_contracts.sql");
const MIGRATION_004: &str = include_str!("004_create_audit_log.sql");
const MIGRATION_005: &str = include_str!("005_create_evidence.sql");
const MIGRATION_006: &str = include_str!("006_create_policy_rules.sql");
const MIGRATION_007: &str = include_str!("007_create_metrics.sql");
const MIGRATION_008: &str = include_str!("008_create_wp_dependencies.sql");
const MIGRATION_009: &str = include_str!("009_create_indexes.sql");
const MIGRATION_010: &str = include_str!("010_create_events.sql");
const MIGRATION_011: &str = include_str!("011_create_snapshots.sql");
const MIGRATION_012: &str = include_str!("012_create_sync_mappings.sql");
const MIGRATION_013: &str = include_str!("013_create_api_keys.sql");
const MIGRATION_014: &str = include_str!("014_create_device_nodes.sql");
const MIGRATION_015: &str = include_str!("015_modules_cycles.sql");
const MIGRATION_016: &str = include_str!("016_create_backlog_items.sql");
const MIGRATION_017: &str = include_str!("017_create_projects.sql");
const MIGRATION_018: &str = include_str!("018_create_users.sql");
const MIGRATION_019: &str = include_str!("019_create_epics.sql");
const MIGRATION_020: &str = include_str!("020_create_stories.sql");
const MIGRATION_021: &str = include_str!("021_add_requirement_id.sql");
const MIGRATION_022: &str = include_str!("022_create_trace_links.sql");
const MIGRATION_023: &str = include_str!("023_create_worklog_entries.sql");
const MIGRATION_024: &str = include_str!("024_l2_38_worklog_trace_gate_run_scope.sql");
const MIGRATION_025: &str = include_str!("025_create_intent_graph.sql");
const MIGRATION_025_GOV: &str = include_str!("025_governance_channel_iteration.sql");
const MIGRATION_025_VIEWS: &str = include_str!("025_intent_graph_views.sql");
const MIGRATION_026: &str = include_str!("026_feature_labels.sql");

/// All migrations in order: (name, up_sql, down_sql)
const MIGRATIONS: &[(&str, &str)] = &[
    ("001_create_features", MIGRATION_001),
    ("002_create_work_packages", MIGRATION_002),
    ("003_create_governance_contracts", MIGRATION_003),
    ("004_create_audit_log", MIGRATION_004),
    ("005_create_evidence", MIGRATION_005),
    ("006_create_policy_rules", MIGRATION_006),
    ("007_create_metrics", MIGRATION_007),
    ("008_create_wp_dependencies", MIGRATION_008),
    ("009_create_indexes", MIGRATION_009),
    ("010_create_events", MIGRATION_010),
    ("011_create_snapshots", MIGRATION_011),
    ("012_create_sync_mappings", MIGRATION_012),
    ("013_create_api_keys", MIGRATION_013),
    ("014_create_device_nodes", MIGRATION_014),
    ("015_modules_cycles", MIGRATION_015),
    ("016_create_backlog_items", MIGRATION_016),
    ("017_create_projects", MIGRATION_017),
    ("018_create_users", MIGRATION_018),
    ("019_create_epics", MIGRATION_019),
    ("020_create_stories", MIGRATION_020),
    ("021_add_requirement_id", MIGRATION_021),
    ("022_create_trace_links", MIGRATION_022),
    ("023_create_worklog_entries", MIGRATION_023),
    ("024_l2_38_worklog_trace_gate_run_scope", MIGRATION_024),
    ("025_create_intent_graph", MIGRATION_025),
    ("025_governance_channel_iteration", MIGRATION_025_GOV),
    ("025_intent_graph_views", MIGRATION_025_VIEWS),
    ("026_feature_labels", MIGRATION_026),
];

/// Find the byte offset where the UP body starts, given a `-- UP` marker
/// (the `UP` token may be followed by `:`, whitespace, or anything). Returns
/// `None` if no UP marker is present.
fn find_up_body_start(sql: &str) -> Option<usize> {
    // Look for `-- UP` not followed by a lowercase letter (avoids matching
    // `-- UPGRADE` etc); allows `-- UP`, `-- UP:`, `-- UP --` ...
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i + 5 <= bytes.len() {
        if &bytes[i..i + 5] == b"-- UP"
            && (i + 5 == bytes.len()
                || !bytes[i + 5].is_ascii_lowercase())
        {
            // Skip the marker + any trailing `:`, whitespace, or `--` (line comment).
            let mut j = i + 5;
            // Single trailing ':'
            if j < bytes.len() && bytes[j] == b':' { j += 1; }
            // Skip rest of the line (the marker comment)
            while j < bytes.len() && bytes[j] != b'\n' { j += 1; }
            // Skip the newline
            if j < bytes.len() { j += 1; }
            return Some(j);
        }
        i += 1;
    }
    None
}

/// Parse the UP section from a migration SQL file.
fn parse_up(sql: &str) -> &str {
    // Format is:
    //   -- UP [-- up to text]
    //   <sql>
    //   -- DOWN
    //   <sql>
    if let Some(up_start) = find_up_body_start(sql) {
        if let Some(down_start) = sql[up_start..].find("-- DOWN") {
            return sql[up_start..up_start + down_start].trim();
        }
        return sql[up_start..].trim();
    }
    sql.trim()
}

/// Parse the DOWN section from a migration SQL file.
fn parse_down(sql: &str) -> &str {
    if let Some(down_start) = sql.find("-- DOWN") {
        return sql[down_start + 7..].trim();
    }
    ""
}

fn map_err(e: rusqlite::Error) -> DomainError {
    DomainError::Storage(e.to_string())
}

/// Runs database schema migrations.
pub struct MigrationRunner<'a> {
    conn: &'a Connection,
}

impl<'a> MigrationRunner<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create the migrations tracking table if it doesn't exist.
    fn ensure_meta_table(&self) -> Result<(), DomainError> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS _migrations (
                    id         INTEGER PRIMARY KEY AUTOINCREMENT,
                    name       TEXT    UNIQUE NOT NULL,
                    applied_at TEXT    NOT NULL
                );",
            )
            .map_err(map_err)
    }

    /// Check whether a migration has already been applied.
    fn is_applied(&self, name: &str) -> SqlResult<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM _migrations WHERE name = ?1",
            rusqlite::params![name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Apply all pending migrations in order.
    pub fn run_all(&self) -> Result<(), DomainError> {
        self.ensure_meta_table()?;

        for (name, sql) in MIGRATIONS {
            if self.is_applied(name).map_err(map_err)? {
                continue;
            }

            let up_sql = parse_up(sql);
            self.conn
                .execute_batch(up_sql)
                .map_err(|e| DomainError::Storage(format!("migration {name} failed: {e}")))?;

            let now = chrono::Utc::now().to_rfc3339();
            self.conn
                .execute(
                    "INSERT INTO _migrations (name, applied_at) VALUES (?1, ?2)",
                    rusqlite::params![name, now],
                )
                .map_err(map_err)?;
        }

        Ok(())
    }

    /// Roll back the most recently applied migration.
    pub fn rollback_last(&self) -> Result<(), DomainError> {
        self.ensure_meta_table()?;

        let last_name: Option<String> = self
            .conn
            .query_row(
                "SELECT name FROM _migrations ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_err)?;

        let Some(name) = last_name else {
            return Ok(()); // Nothing to roll back
        };

        // Find the migration SQL
        let migration = MIGRATIONS.iter().find(|(n, _)| *n == name.as_str());
        if let Some((_, sql)) = migration {
            let down_sql = parse_down(sql);
            if !down_sql.is_empty() {
                self.conn
                    .execute_batch(down_sql)
                    .map_err(|e| DomainError::Storage(format!("rollback of {name} failed: {e}")))?;
            }
        }

        self.conn
            .execute(
                "DELETE FROM _migrations WHERE name = ?1",
                rusqlite::params![name],
            )
            .map_err(map_err)?;

        Ok(())
    }
}

/// Extension trait to add `.optional()` on rusqlite query results.
trait OptionalExt<T> {
    fn optional(self) -> SqlResult<Option<T>>;
}

impl<T> OptionalExt<T> for SqlResult<T> {
    fn optional(self) -> SqlResult<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_include_016_create_backlog_items() {
        assert!(
            MIGRATIONS
                .iter()
                .any(|(name, _)| *name == "016_create_backlog_items"),
            "016_create_backlog_items must stay registered so default DBs heal on open"
        );
    }

    #[test]
    fn run_all_creates_backlog_items_table() {
        let conn = Connection::open_in_memory().expect("in-memory");
        MigrationRunner::new(&conn).run_all().expect("migrate");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'backlog_items'",
                [],
                |row| row.get(0),
            )
            .expect("query");
        assert_eq!(count, 1, "backlog_items must exist after run_all");
    }
}
