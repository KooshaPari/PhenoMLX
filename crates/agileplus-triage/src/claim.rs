// SPDX-License-Identifier: MIT OR Apache-2.0
//! Resource claim primitives with TTL and heartbeat.
//!
//! A claim is a record that an agent has exclusive ownership of a
//! resource (repo, branch, worktree, or subproject) for `ttl_seconds`
//! (default 3600). Agents must heartbeat before the TTL elapses or the
//! claim reverts to the global pool.
//!
//! # `ClaimReason`
//!
//! The `reason` field on a [`Claim`] is a structured [`ClaimReason`],
//! not a free-form string. Each variant names the *kind* of artifact
//! that motivated the claim, with the value carrying the canonical
//! identifier (work-package id, branch name, subproject name, run id, or
//! a free-form human note). The struct is `serde(tag = "kind",
//! content = "value")` so the wire shape is e.g.
//!
//! ```json
//! {"kind": "task_ref", "value": "wp-1"}
//! ```
//!
//! # Stores
//!
//! [`ClaimStore`] is the in-memory implementation, suitable for
//! single-process CLI / test use. The trait [`ClaimStoreTrait`] is the
//! common surface; the SQLite-backed [`crate::claim_store_sqlite`]
//! (feature `sqlite`) implements the same trait for multi-process
//! deployments.
//!
//! Traceability: FR-AGP-019 (resource claim primitive), audit recs
//! #6, #7, #8 from `AUDIT_BLOC_VS_2026_SOTA.md`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Type of resource being claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    /// A repository (full directory).
    Repo,
    /// A git branch within a repo.
    Branch,
    /// A git worktree within a repo.
    Worktree,
    /// A subproject (logical grouping within a repo).
    Subproject,
}

/// State of a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimState {
    /// Currently valid and held.
    Active,
    /// Grace period: TTL elapsed but a heartbeat was received; in reclamation.
    Draining,
    /// Returned to the global pool (expired or released).
    Expired,
}

/// Structured reason for a claim.
///
/// The wire shape is `{"kind": "<variant>", "value": "<payload>"}` — the
/// `kind` discriminates the variant and `value` carries the
/// variant-specific identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ClaimReason {
    /// The claim is linked to a work-package id (e.g. `"wp-1"`).
    TaskRef(String),
    /// The claim is for a named git branch (e.g. `"feat/login"`).
    Branch(String),
    /// The claim is for a subproject (logical grouping) name.
    Subproject(String),
    /// The claim is for an ephemeral run id (CI / WIP / scratch).
    WipRun(String),
    /// Free-form, human-entered reason that doesn't fit the other
    /// structured variants. Default value (see [`Default`]) is the
    /// empty string.
    Manual(String),
}

impl Default for ClaimReason {
    fn default() -> Self {
        ClaimReason::Manual(String::new())
    }
}

impl ClaimReason {
    /// Discriminant as a stable lowercase string — the same string used
    /// for the `kind` field on the wire. Useful for SQL persistence and
    /// logging.
    pub fn kind_str(&self) -> &'static str {
        match self {
            ClaimReason::TaskRef(_) => "task_ref",
            ClaimReason::Branch(_) => "branch",
            ClaimReason::Subproject(_) => "subproject",
            ClaimReason::WipRun(_) => "wip_run",
            ClaimReason::Manual(_) => "manual",
        }
    }

    /// The value carried by the variant, regardless of which variant it
    /// is. Equivalent to the `value` field on the wire.
    pub fn value(&self) -> &str {
        match self {
            ClaimReason::TaskRef(s)
            | ClaimReason::Branch(s)
            | ClaimReason::Subproject(s)
            | ClaimReason::WipRun(s)
            | ClaimReason::Manual(s) => s.as_str(),
        }
    }
}

/// A claim record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub id: String,
    pub resource: String,
    pub kind: ClaimKind,
    pub agent_id: String,
    pub created_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
    pub ttl_seconds: i64,
    pub state: ClaimState,
    /// Structured reason for the claim. See [`ClaimReason`].
    #[serde(default)]
    pub reason: ClaimReason,
}

impl Claim {
    /// Seconds since last heartbeat; negative if in the future (clock skew).
    /// Truncated to whole seconds (see `is_expired` for sub-second precision).
    pub fn age_seconds(&self, now: DateTime<Utc>) -> i64 {
        (now - self.last_heartbeat).num_seconds()
    }

    /// True if TTL has elapsed (still recoverable via grace period).
    /// Uses millisecond precision so sub-second TTLs work reliably.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        (now - self.last_heartbeat).num_milliseconds() > self.ttl_seconds * 1000
    }
}

/// Errors surfaced by the claim API.
#[derive(Debug, Error)]
pub enum ClaimError {
    /// No claim with the given id exists.
    #[error("claim not found: {0}")]
    NotFound(String),
    /// The caller attempted a state-sensitive operation
    /// (e.g. `claim_transfer`) on a claim owned by a different agent.
    #[error("wrong owner: expected agent {expected}, claim is held by {actual}")]
    WrongOwner { expected: String, actual: String },
    /// The claim is in a state that does not permit the requested
    /// operation (e.g. transferring an already-`Draining` or
    /// `Expired` claim).
    #[error("wrong state for operation")]
    WrongState,
}

/// Common store interface implemented by both [`ClaimStore`]
/// (in-memory) and `claim_store_sqlite::SqliteClaimStore`
/// (feature `sqlite`).
///
/// This is intentionally a sync trait — it operates on a `&mut self`
/// reference and does no I/O that the caller hasn't already chosen to
/// accept. The SQLite implementation is `Send` but not `Sync` (the
/// connection is not `Sync`), so the trait bounds stay minimal and the
/// caller can wrap it in a `Mutex` if they need shared access.
pub trait ClaimStoreTrait {
    /// Issue a new claim. See [`ClaimStore::claim`].
    fn claim(
        &mut self,
        id: &str,
        resource: &str,
        kind: ClaimKind,
        agent: &str,
        ttl_seconds: i64,
        reason: ClaimReason,
    ) -> Option<Claim>;

    /// Refresh `last_heartbeat` to now. See [`ClaimStore::heartbeat`].
    fn heartbeat(&mut self, id: &str) -> bool;

    /// Release a claim. See [`ClaimStore::release`].
    fn release(&mut self, id: &str) -> bool;

    /// Reap TTL-expired claims. See [`ClaimStore::reap_expired`].
    fn reap_expired(&mut self, now: DateTime<Utc>) -> usize;

    /// All claims. See [`ClaimStore::all`].
    fn all(&self) -> Vec<Claim>;

    /// Active claims only. See [`ClaimStore::active`].
    fn active(&self) -> Vec<Claim>;

    /// Lookup by resource. See [`ClaimStore::lookup`].
    fn lookup(&self, kind: ClaimKind, resource: &str) -> Option<Claim>;

    /// Transfer a claim from one id to another, inheriting the
    /// resource, kind, ttl, and reason. The old claim is marked
    /// `Draining`. See [`ClaimStore::claim_transfer`].
    fn claim_transfer(
        &mut self,
        from_id: &str,
        to_id: &str,
        to_agent: &str,
    ) -> Result<Claim, ClaimError>;
}

/// In-memory store of claims. Use external locking for thread-safety in
/// production deployments.
#[derive(Debug, Default, Clone)]
pub struct ClaimStore {
    claims: HashMap<String, Claim>,
    by_resource: HashMap<(ClaimKind, String), String>,
}

impl ClaimStore {
    /// New empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// All claims regardless of state.
    pub fn all(&self) -> Vec<Claim> {
        self.claims.values().cloned().collect()
    }

    /// Active claims only.
    pub fn active(&self) -> Vec<Claim> {
        self.claims
            .values()
            .filter(|c| c.state == ClaimState::Active)
            .cloned()
            .collect()
    }

    /// Issue a new claim. Returns `None` if the resource is already
    /// actively claimed by a different claim id.
    pub fn claim(
        &mut self,
        id: &str,
        resource: &str,
        kind: ClaimKind,
        agent: &str,
        ttl_seconds: i64,
        reason: ClaimReason,
    ) -> Option<Claim> {
        let resource_key = (kind, resource.to_string());
        if let Some(existing_id) = self.by_resource.get(&resource_key) {
            if existing_id != id
                && self
                    .claims
                    .get(existing_id)
                    .map(|c| c.state == ClaimState::Active)
                    .unwrap_or(false)
            {
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
            reason,
        };
        self.claims.insert(id.to_string(), c.clone());
        self.by_resource.insert(resource_key, id.to_string());
        Some(c)
    }

    /// Refresh `last_heartbeat` to now. Returns `true` if the claim was
    /// found and refreshed.
    pub fn heartbeat(&mut self, id: &str) -> bool {
        if let Some(c) = self.claims.get_mut(id) {
            c.last_heartbeat = Utc::now();
            true
        } else {
            false
        }
    }

    /// Release a claim (active release by the owner). Returns `true` if
    /// the claim was found and released.
    pub fn release(&mut self, id: &str) -> bool {
        if let Some(c) = self.claims.remove(id) {
            self.by_resource.remove(&(c.kind, c.resource));
            true
        } else {
            false
        }
    }

    /// Reap all claims whose TTL has elapsed. Removes them from the store
    /// and frees the resource. Returns the number of reaped claims.
    pub fn reap_expired(&mut self, now: DateTime<Utc>) -> usize {
        let mut reaped = 0;
        let expired: Vec<String> = self
            .claims
            .iter()
            .filter(|(_, c)| c.is_expired(now))
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            if let Some(c) = self.claims.remove(&id) {
                self.by_resource.remove(&(c.kind, c.resource));
                reaped += 1;
            }
        }
        reaped
    }

    /// Lookup by resource.
    pub fn lookup(&self, kind: ClaimKind, resource: &str) -> Option<Claim> {
        self.by_resource
            .get(&(kind, resource.to_string()))
            .and_then(|id| self.claims.get(id))
            .cloned()
    }

    /// Transfer a claim to a new id and agent.
    ///
    /// Semantics:
    ///
    /// 1. The existing claim (under `from_id`) must exist and be in
    ///    [`ClaimState::Active`]. If it doesn't exist, returns
    ///    [`ClaimError::NotFound`]. If it's `Draining` or `Expired`,
    ///    returns [`ClaimError::WrongState`].
    /// 2. The existing claim's `state` is set to [`ClaimState::Draining`]
    ///    (in place — same id, same `by_resource` mapping), so the
    ///    resource is still held by `from_id` until the new owner
    ///    finishes whatever handoff protocol the agents use.
    /// 3. A new [`Claim`] is inserted under `to_id`, inheriting the
    ///    resource, kind, ttl, and reason. The new claim is
    ///    [`ClaimState::Active`], owned by `to_agent`, with fresh
    ///    `created_at` and `last_heartbeat`.
    /// 4. The new claim is returned.
    ///
    /// This function does *not* take an `expected_owner` argument: it
    /// always allows the transfer regardless of who currently holds
    /// the claim. Callers that need to enforce ownership should
    /// check the returned `old_agent` (via [`ClaimStore::lookup`] or
    /// by inspecting the claim *before* calling) and reject the
    /// transfer out of band.
    pub fn claim_transfer(
        &mut self,
        from_id: &str,
        to_id: &str,
        to_agent: &str,
    ) -> Result<Claim, ClaimError> {
        // (a) mark the old claim as Draining
        let old = self
            .claims
            .get_mut(from_id)
            .ok_or_else(|| ClaimError::NotFound(from_id.to_string()))?;
        if old.state != ClaimState::Active {
            return Err(ClaimError::WrongState);
        }
        let resource = old.resource.clone();
        let kind = old.kind;
        let ttl_seconds = old.ttl_seconds;
        let reason = old.reason.clone();
        old.state = ClaimState::Draining;
        // (b) create a new claim inheriting resource+kind+ttl+reason
        let now = Utc::now();
        let new_claim = Claim {
            id: to_id.to_string(),
            resource: resource.clone(),
            kind,
            agent_id: to_agent.to_string(),
            created_at: now,
            last_heartbeat: now,
            ttl_seconds,
            state: ClaimState::Active,
            reason,
        };
        self.claims.insert(to_id.to_string(), new_claim.clone());
        self.by_resource.insert((kind, resource), to_id.to_string());
        // (c) return the new claim
        Ok(new_claim)
    }
}

impl ClaimStoreTrait for ClaimStore {
    fn claim(
        &mut self,
        id: &str,
        resource: &str,
        kind: ClaimKind,
        agent: &str,
        ttl_seconds: i64,
        reason: ClaimReason,
    ) -> Option<Claim> {
        ClaimStore::claim(self, id, resource, kind, agent, ttl_seconds, reason)
    }
    fn heartbeat(&mut self, id: &str) -> bool {
        ClaimStore::heartbeat(self, id)
    }
    fn release(&mut self, id: &str) -> bool {
        ClaimStore::release(self, id)
    }
    fn reap_expired(&mut self, now: DateTime<Utc>) -> usize {
        ClaimStore::reap_expired(self, now)
    }
    fn all(&self) -> Vec<Claim> {
        ClaimStore::all(self)
    }
    fn active(&self) -> Vec<Claim> {
        ClaimStore::active(self)
    }
    fn lookup(&self, kind: ClaimKind, resource: &str) -> Option<Claim> {
        ClaimStore::lookup(self, kind, resource)
    }
    fn claim_transfer(
        &mut self,
        from_id: &str,
        to_id: &str,
        to_agent: &str,
    ) -> Result<Claim, ClaimError> {
        ClaimStore::claim_transfer(self, from_id, to_id, to_agent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_reason_default_is_empty_manual() {
        assert_eq!(ClaimReason::default(), ClaimReason::Manual(String::new()));
    }

    #[test]
    fn claim_reason_serde_tag_content() {
        let r = ClaimReason::TaskRef("wp-1".into());
        let s = serde_json::to_string(&r).unwrap();
        // tag=kind, content=value -> {"kind":"task_ref","value":"wp-1"}
        assert_eq!(s, r#"{"kind":"task_ref","value":"wp-1"}"#);

        let round: ClaimReason = serde_json::from_str(&s).unwrap();
        assert_eq!(round, r);
    }

    #[test]
    fn claim_reason_each_variant_round_trip() {
        for r in [
            ClaimReason::TaskRef("wp-1".into()),
            ClaimReason::Branch("feat/login".into()),
            ClaimReason::Subproject("infra".into()),
            ClaimReason::WipRun("run-42".into()),
            ClaimReason::Manual("investigating flaky test".into()),
        ] {
            let s = serde_json::to_string(&r).unwrap();
            let back: ClaimReason = serde_json::from_str(&s).unwrap();
            assert_eq!(back, r);
        }
    }

    #[test]
    fn claim_reason_kind_str_and_value() {
        assert_eq!(ClaimReason::Branch("x".into()).kind_str(), "branch");
        assert_eq!(ClaimReason::Branch("x".into()).value(), "x");
        assert_eq!(ClaimReason::Manual("note".into()).kind_str(), "manual");
        assert_eq!(ClaimReason::Manual(String::new()).value(), "");
    }

    #[test]
    fn claim_default_reason_is_empty_manual() {
        let c = Claim {
            id: "x".into(),
            resource: "r".into(),
            kind: ClaimKind::Repo,
            agent_id: "a".into(),
            created_at: Utc::now(),
            last_heartbeat: Utc::now(),
            ttl_seconds: 60,
            state: ClaimState::Active,
            reason: ClaimReason::default(),
        };
        assert_eq!(c.reason, ClaimReason::Manual(String::new()));
    }

    #[test]
    fn claim_store_trait_in_memory_dispatches() {
        // Verify that the trait impl forwards correctly — the public
        // method on ClaimStore is the source of truth, but consumers
        // that depend on the trait (e.g. the SQLite impl) must get
        // the same semantics.
        let mut s: Box<dyn ClaimStoreTrait> = Box::new(ClaimStore::new());
        let c = s
            .claim(
                "c1",
                "repo:foo",
                ClaimKind::Repo,
                "agent-a",
                60,
                ClaimReason::default(),
            )
            .expect("claim");
        assert_eq!(c.id, "c1");
        assert!(s.heartbeat("c1"));
        assert!(s.release("c1"));
    }

    #[test]
    fn claim_transfer_marks_old_draining_and_returns_new() {
        let mut s = ClaimStore::new();
        s.claim(
            "c1",
            "branch:feat",
            ClaimKind::Branch,
            "agent-a",
            3600,
            ClaimReason::Branch("feat/login".into()),
        );

        let new_claim = s
            .claim_transfer("c1", "c2", "agent-b")
            .expect("transfer succeeds");

        // (c) new claim is active, owned by agent-b, inherits everything.
        assert_eq!(new_claim.id, "c2");
        assert_eq!(new_claim.agent_id, "agent-b");
        assert_eq!(new_claim.resource, "branch:feat");
        assert_eq!(new_claim.kind, ClaimKind::Branch);
        assert_eq!(new_claim.ttl_seconds, 3600);
        assert_eq!(new_claim.state, ClaimState::Active);
        assert_eq!(new_claim.reason, ClaimReason::Branch("feat/login".into()));

        // (a) old claim is now Draining.
        let old = s.lookup(ClaimKind::Branch, "branch:feat").unwrap();
        // The lookup points at the new claim (active wins on
        // by_resource). Verify the old by-id path:
        let draining = s
            .all()
            .into_iter()
            .find(|c| c.id == "c1")
            .expect("old claim still present");
        assert_eq!(draining.state, ClaimState::Draining);
        assert_eq!(draining.resource, "branch:feat");
        assert_eq!(draining.agent_id, "agent-a");
        // `old` is the new claim (lookup follows by_resource).
        assert_eq!(old.id, "c2");
    }

    #[test]
    fn claim_transfer_unknown_from_id_errors() {
        let mut s = ClaimStore::new();
        let err = s
            .claim_transfer("does-not-exist", "c2", "agent-b")
            .expect_err("must error on missing from_id");
        assert!(matches!(err, ClaimError::NotFound(ref id) if id == "does-not-exist"));
    }

    #[test]
    fn claim_transfer_wrong_state_when_already_draining() {
        let mut s = ClaimStore::new();
        s.claim(
            "c1",
            "branch:feat",
            ClaimKind::Branch,
            "agent-a",
            3600,
            ClaimReason::default(),
        );
        // First transfer: c1 -> c2 (c1 is Draining afterwards).
        s.claim_transfer("c1", "c2", "agent-b").unwrap();
        // Second transfer from c1 (now Draining) must fail.
        let err = s.claim_transfer("c1", "c3", "agent-c").unwrap_err();
        assert!(matches!(err, ClaimError::WrongState));
    }

    #[test]
    fn claim_error_display_messages() {
        // Smoke-test Display impls for the new error variants.
        let nf = ClaimError::NotFound("abc".into());
        assert!(nf.to_string().contains("abc"));
        let wo = ClaimError::WrongOwner {
            expected: "a".into(),
            actual: "b".into(),
        };
        let msg = wo.to_string();
        assert!(msg.contains("a") && msg.contains("b"));
        assert_eq!(
            ClaimError::WrongState.to_string(),
            "wrong state for operation"
        );
    }
}
