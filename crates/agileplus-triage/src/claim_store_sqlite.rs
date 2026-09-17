// SPDX-License-Identifier: MIT OR Apache-2.0
//! SQLite-backed [`ClaimStore`](super::ClaimStore) implementation.
//!
//! Enabled by the `sqlite` Cargo feature. Persists claims to a
//! [`rusqlite::Connection`], so it is suitable for multi-process
//! deployments where a single in-memory store would lose state across
//! process restarts.
//!
//! # Schema
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS claims (
//!   id TEXT PRIMARY KEY,
//!   resource TEXT NOT NULL,
//!   kind TEXT NOT NULL,
//!   agent_id TEXT NOT NULL,
//!   created_at TEXT NOT NULL,
//!   last_heartbeat TEXT NOT NULL,
//!   ttl_seconds INTEGER NOT NULL,
//!   state TEXT NOT NULL,
//!   reason_kind TEXT,
//!   reason_value TEXT
//! );
//! CREATE INDEX IF NOT EXISTS idx_claims_resource ON claims(resource, kind);
//! CREATE INDEX IF NOT EXISTS idx_claims_state ON claims(state);
//! ```
//!
//! The `reason` column is split into `reason_kind` and `reason_value`
//! so that callers can filter / group by `reason_kind` in SQL without
//! parsing the JSON.
//!
//! # Concurrency
//!
//! The struct is `Send` (the connection is `Send`) but not `Sync` (the
//! connection is not `Sync`). Wrap in a `Mutex` for shared access.
//!
//! Traceability: audit rec #8 from `AUDIT_BLOC_VS_2026_SOTA.md`.

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::claim::{Claim, ClaimError, ClaimKind, ClaimReason, ClaimState, ClaimStoreTrait};

/// Mapping error from a SQLite-level error into the claim domain.
#[derive(Debug, thiserror::Error)]
pub enum SqliteClaimStoreError {
    #[error("rusqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("unknown claim kind: {0}")]
    UnknownKind(String),

    #[error("unknown claim state: {0}")]
    UnknownState(String),
}

impl From<SqliteClaimStoreError> for ClaimError {
    fn from(e: SqliteClaimStoreError) -> Self {
        // The only `ClaimError` variant that can surface from this
        // store's `claim_transfer` path is `NotFound`; for everything
        // else, the underlying rusqlite error becomes a generic
        // `NotFound("...storage error...")` so the trait signature stays
        // compact. Callers needing the structured error can construct
        // the store directly and use its methods.
        match e {
            SqliteClaimStoreError::Sqlite(err) => {
                ClaimError::NotFound(format!("storage error: {err}"))
            }
            other => ClaimError::NotFound(other.to_string()),
        }
    }
}

/// SQLite-backed claim store. Implements [`ClaimStoreTrait`].
pub struct SqliteClaimStore {
    conn: Connection,
}

impl SqliteClaimStore {
    /// Open (or create) the store at the given path.
    pub fn open(path: &str) -> Result<Self, SqliteClaimStoreError> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Open an in-memory database. Useful for tests.
    pub fn open_in_memory() -> Result<Self, SqliteClaimStoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), SqliteClaimStoreError> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS claims (
              id TEXT PRIMARY KEY,
              resource TEXT NOT NULL,
              kind TEXT NOT NULL,
              agent_id TEXT NOT NULL,
              created_at TEXT NOT NULL,
              last_heartbeat TEXT NOT NULL,
              ttl_seconds INTEGER NOT NULL,
              state TEXT NOT NULL,
              reason_kind TEXT,
              reason_value TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_claims_resource ON claims(resource, kind);
            CREATE INDEX IF NOT EXISTS idx_claims_state ON claims(state);
            "#,
        )?;
        Ok(())
    }
}

impl SqliteClaimStore {
    fn kind_to_str(k: ClaimKind) -> &'static str {
        match k {
            ClaimKind::Repo => "repo",
            ClaimKind::Branch => "branch",
            ClaimKind::Worktree => "worktree",
            ClaimKind::Subproject => "subproject",
        }
    }

    fn str_to_kind(s: &str) -> Result<ClaimKind, SqliteClaimStoreError> {
        match s {
            "repo" => Ok(ClaimKind::Repo),
            "branch" => Ok(ClaimKind::Branch),
            "worktree" => Ok(ClaimKind::Worktree),
            "subproject" => Ok(ClaimKind::Subproject),
            other => Err(SqliteClaimStoreError::UnknownKind(other.to_string())),
        }
    }

    fn state_to_str(s: ClaimState) -> &'static str {
        match s {
            ClaimState::Active => "active",
            ClaimState::Draining => "draining",
            ClaimState::Expired => "expired",
        }
    }

    fn str_to_state(s: &str) -> Result<ClaimState, SqliteClaimStoreError> {
        match s {
            "active" => Ok(ClaimState::Active),
            "draining" => Ok(ClaimState::Draining),
            "expired" => Ok(ClaimState::Expired),
            other => Err(SqliteClaimStoreError::UnknownState(other.to_string())),
        }
    }

    fn parse_ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .map(|dt| Utc.from_utc_datetime(&dt.naive_utc()))
            .unwrap_or_else(|_| Utc::now())
    }

    fn fmt_ts(t: DateTime<Utc>) -> String {
        t.to_rfc3339()
    }

    fn row_to_claim(row: &Row<'_>) -> Result<Claim, rusqlite::Error> {
        let id: String = row.get("id")?;
        let resource: String = row.get("resource")?;
        let kind_str: String = row.get("kind")?;
        let kind = Self::str_to_kind(&kind_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::other(e.to_string())),
            )
        })?;
        let agent_id: String = row.get("agent_id")?;
        let created_at_str: String = row.get("created_at")?;
        let last_heartbeat_str: String = row.get("last_heartbeat")?;
        let ttl_seconds: i64 = row.get("ttl_seconds")?;
        let state_str: String = row.get("state")?;
        let state = Self::str_to_state(&state_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::other(e.to_string())),
            )
        })?;
        let reason_kind: Option<String> = row.get("reason_kind")?;
        let reason_value: Option<String> = row.get("reason_value")?;
        let reason = match (reason_kind, reason_value) {
            (Some(k), v) => match k.as_str() {
                "task_ref" => ClaimReason::TaskRef(v.unwrap_or_default()),
                "branch" => ClaimReason::Branch(v.unwrap_or_default()),
                "subproject" => ClaimReason::Subproject(v.unwrap_or_default()),
                "wip_run" => ClaimReason::WipRun(v.unwrap_or_default()),
                // Unknown kind - fall back to Manual so the row round-trips
                // even after a schema migration introduces a new variant.
                _ => ClaimReason::Manual(v.unwrap_or_default()),
            },
            (None, _) => ClaimReason::default(),
        };
        Ok(Claim {
            id,
            resource,
            kind,
            agent_id,
            created_at: Self::parse_ts(&created_at_str),
            last_heartbeat: Self::parse_ts(&last_heartbeat_str),
            ttl_seconds,
            state,
            reason,
        })
    }
}

impl ClaimStoreTrait for SqliteClaimStore {
    fn claim(
        &mut self,
        id: &str,
        resource: &str,
        kind: ClaimKind,
        agent: &str,
        ttl_seconds: i64,
        reason: ClaimReason,
    ) -> Option<Claim> {
        // Honour the in-memory semantics: if there is already an
        // Active claim for (kind, resource) by a *different* id, refuse.
        if let Some(existing) = self.lookup(kind, resource) {
            if existing.id != id && existing.state == ClaimState::Active {
                return None;
            }
        }
        let now = Utc::now();
        let c = Claim {
            id: id.to_string(),
            resource: resource.to_string(),
            kind,
            agent_id: agent.to_string(),
            created_at: now,
            last_heartbeat: now,
            ttl_seconds,
            state: ClaimState::Active,
            reason: reason.clone(),
        };
        let kind_str = Self::kind_to_str(kind);
        let state_str = Self::state_to_str(ClaimState::Active);
        let reason_kind = reason.kind_str();
        let reason_value = reason.value();
        // Upsert - replacing the existing row if `id` already exists
        // (e.g. re-claim after release). This matches the spec:
        // "Use INSERT OR REPLACE INTO claims(...) for upserts."
        let res = self.conn.execute(
            "INSERT OR REPLACE INTO claims \
             (id, resource, kind, agent_id, created_at, last_heartbeat, ttl_seconds, state, reason_kind, reason_value) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                resource,
                kind_str,
                agent,
                Self::fmt_ts(now),
                Self::fmt_ts(now),
                ttl_seconds,
                state_str,
                reason_kind,
                reason_value,
            ],
        );
        match res {
            Ok(_) => Some(c),
            Err(_) => None,
        }
    }

    fn heartbeat(&mut self, id: &str) -> bool {
        let now_str = Self::fmt_ts(Utc::now());
        let updated = self
            .conn
            .execute(
                "UPDATE claims SET last_heartbeat = ?1 WHERE id = ?2",
                params![now_str, id],
            )
            .unwrap_or(0);
        updated > 0
    }

    fn release(&mut self, id: &str) -> bool {
        let deleted = self
            .conn
            .execute("DELETE FROM claims WHERE id = ?1", params![id])
            .unwrap_or(0);
        deleted > 0
    }

    fn reap_expired(&mut self, now: DateTime<Utc>) -> usize {
        let mut stmt = match self
            .conn
            .prepare("SELECT id, last_heartbeat, ttl_seconds FROM claims")
        {
            Ok(s) => s,
            Err(_) => return 0,
        };
        let rows = match stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let lh: String = row.get(1)?;
            let ttl: i64 = row.get(2)?;
            Ok((id, lh, ttl))
        }) {
            Ok(r) => r,
            Err(_) => return 0,
        };
        let mut to_delete: Vec<String> = Vec::new();
        for row in rows.flatten() {
            let (id, lh, ttl) = row;
            let lh_dt = Self::parse_ts(&lh);
            if (now - lh_dt).num_milliseconds() > ttl * 1000 {
                to_delete.push(id);
            }
        }
        let mut reaped = 0;
        for id in to_delete {
            let n = self
                .conn
                .execute("DELETE FROM claims WHERE id = ?1", params![id])
                .unwrap_or(0);
            reaped += n;
        }
        reaped
    }

    fn all(&self) -> Vec<Claim> {
        let mut stmt = match self.conn.prepare("SELECT * FROM claims") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([], Self::row_to_claim)
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }

    fn active(&self) -> Vec<Claim> {
        self.all()
            .into_iter()
            .filter(|c| c.state == ClaimState::Active)
            .collect()
    }

    fn lookup(&self, kind: ClaimKind, resource: &str) -> Option<Claim> {
        let kind_str = Self::kind_to_str(kind);
        let mut stmt = match self.conn.prepare(
            "SELECT * FROM claims WHERE kind = ?1 AND resource = ?2 AND state = 'active' LIMIT 1",
        ) {
            Ok(s) => s,
            Err(_) => return None,
        };
        stmt.query_row(params![kind_str, resource], Self::row_to_claim)
            .optional()
            .ok()
            .flatten()
    }

    fn claim_transfer(
        &mut self,
        from_id: &str,
        to_id: &str,
        to_agent: &str,
    ) -> Result<Claim, ClaimError> {
        // Load old claim.
        let mut old_stmt = match self.conn.prepare("SELECT * FROM claims WHERE id = ?1") {
            Ok(s) => s,
            Err(e) => return Err(ClaimError::NotFound(format!("storage error: {e}"))),
        };
        let old: Option<Claim> = old_stmt
            .query_row(params![from_id], Self::row_to_claim)
            .optional()
            .map_err(|e| ClaimError::NotFound(format!("storage error: {e}")))?;
        let mut old = old.ok_or_else(|| ClaimError::NotFound(from_id.to_string()))?;
        if old.state != ClaimState::Active {
            return Err(ClaimError::WrongState);
        }
        // (a) mark the old claim as Draining.
        old.state = ClaimState::Draining;
        let kind_str = Self::kind_to_str(old.kind);
        let state_str = Self::state_to_str(ClaimState::Draining);
        self.conn
            .execute(
                "UPDATE claims SET state = ?1 WHERE id = ?2",
                params![state_str, from_id],
            )
            .map_err(|e| ClaimError::NotFound(format!("storage error: {e}")))?;
        // (b) create a new claim inheriting everything.
        let now = Utc::now();
        let new_claim = Claim {
            id: to_id.to_string(),
            resource: old.resource.clone(),
            kind: old.kind,
            agent_id: to_agent.to_string(),
            created_at: now,
            last_heartbeat: now,
            ttl_seconds: old.ttl_seconds,
            state: ClaimState::Active,
            reason: old.reason.clone(),
        };
        let new_state_str = Self::state_to_str(ClaimState::Active);
        let reason_kind = new_claim.reason.kind_str();
        let reason_value = new_claim.reason.value();
        self.conn
            .execute(
                "INSERT OR REPLACE INTO claims \
                 (id, resource, kind, agent_id, created_at, last_heartbeat, ttl_seconds, state, reason_kind, reason_value) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    to_id,
                    new_claim.resource,
                    kind_str,
                    to_agent,
                    Self::fmt_ts(now),
                    Self::fmt_ts(now),
                    new_claim.ttl_seconds,
                    new_state_str,
                    reason_kind,
                    reason_value,
                ],
            )
            .map_err(|e| ClaimError::NotFound(format!("storage error: {e}")))?;
        // (c) return the new claim.
        Ok(new_claim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_store_claim_and_lookup() {
        let mut s = SqliteClaimStore::open_in_memory().unwrap();
        let c = s
            .claim(
                "c1",
                "repo:foo",
                ClaimKind::Repo,
                "agent-a",
                60,
                ClaimReason::TaskRef("wp-1".into()),
            )
            .expect("claim");
        assert_eq!(c.id, "c1");
        assert_eq!(c.reason, ClaimReason::TaskRef("wp-1".into()));

        let found = s.lookup(ClaimKind::Repo, "repo:foo").unwrap();
        assert_eq!(found.agent_id, "agent-a");
        assert_eq!(found.reason, ClaimReason::TaskRef("wp-1".into()));
    }

    #[test]
    fn sqlite_store_conflict_on_active() {
        let mut s = SqliteClaimStore::open_in_memory().unwrap();
        assert!(s
            .claim(
                "c1",
                "repo:foo",
                ClaimKind::Repo,
                "agent-a",
                60,
                ClaimReason::default()
            )
            .is_some());
        assert!(s
            .claim(
                "c2",
                "repo:foo",
                ClaimKind::Repo,
                "agent-b",
                60,
                ClaimReason::default()
            )
            .is_none());
    }

    #[test]
    fn sqlite_store_heartbeat_and_release() {
        let mut s = SqliteClaimStore::open_in_memory().unwrap();
        s.claim(
            "c1",
            "branch:feat",
            ClaimKind::Branch,
            "agent-a",
            60,
            ClaimReason::default(),
        )
        .unwrap();
        assert!(s.heartbeat("c1"));
        assert!(s.release("c1"));
        assert!(!s.heartbeat("c1"));
    }

    #[test]
    fn sqlite_store_reap_expired() {
        let mut s = SqliteClaimStore::open_in_memory().unwrap();
        s.claim(
            "c1",
            "branch:feat",
            ClaimKind::Branch,
            "agent-a",
            0,
            ClaimReason::default(),
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let reaped = s.reap_expired(Utc::now());
        assert_eq!(reaped, 1);
    }

    #[test]
    fn sqlite_store_transfer() {
        let mut s = SqliteClaimStore::open_in_memory().unwrap();
        s.claim(
            "c1",
            "branch:feat",
            ClaimKind::Branch,
            "agent-a",
            3600,
            ClaimReason::Branch("feat/login".into()),
        )
        .unwrap();
        let new_claim = s.claim_transfer("c1", "c2", "agent-b").unwrap();
        assert_eq!(new_claim.id, "c2");
        assert_eq!(new_claim.agent_id, "agent-b");
        assert_eq!(new_claim.reason, ClaimReason::Branch("feat/login".into()));

        let all = s.all();
        let old = all.iter().find(|c| c.id == "c1").unwrap();
        assert_eq!(old.state, ClaimState::Draining);
    }

    #[test]
    fn sqlite_store_transfer_not_found() {
        let mut s = SqliteClaimStore::open_in_memory().unwrap();
        let err = s.claim_transfer("missing", "c2", "agent-b").unwrap_err();
        assert!(matches!(err, ClaimError::NotFound(_)));
    }
}
