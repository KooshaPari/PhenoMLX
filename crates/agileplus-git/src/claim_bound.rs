//! Claim-bound worktree creation (audit rec #11).
//!
//! [`ClaimBoundWorktree::create`] is the single entry point for
//! "create a worktree for this Claim". It enforces three invariants
//! that the bare [`super::GitVcsAdapter::create_worktree`] does not:
//!
//! 1. The [`Claim`] must be `kind == ClaimKind::Worktree` and `state
//!    == ClaimState::Active`. Anything else is
//!    [`DomainError::InvalidClaim`].
//! 2. The worktree is actually created on disk (via the underlying
//!    `create_worktree`).
//! 3. The resulting path is recorded on the claim by replacing its
//!    [`ClaimReason`] with [`ClaimReason::Branch`] holding the
//!    canonical `feat/<feature_slug>/<wp_id>` branch name (the same
//!    branch the worktree was checked out on). A later caller can
//!    look up the worktree path with [`ClaimBoundWorktree::lookup`].

use std::path::PathBuf;

use agileplus_domain::error::DomainError;
use agileplus_domain::ports::vcs::VcsPort;
use agileplus_triage::claim::{
    Claim, ClaimError, ClaimKind, ClaimReason, ClaimState, ClaimStoreTrait,
};

use super::GitVcsAdapter;

/// Subset of [`ClaimStoreTrait`] we need to update a claim's reason
/// after a successful worktree creation. The blanket `dyn
/// ClaimStoreTrait` already covers this, but spelling out the bound
/// makes the integration with [`ClaimBoundWorktree`] explicit and
/// keeps the trait import surface minimal.
pub trait ClaimStoreBound: ClaimStoreTrait {}
impl<T: ClaimStoreTrait + ?Sized> ClaimStoreBound for T {}

/// High-level "create a worktree, bound to a claim" helper.
pub struct ClaimBoundWorktree;

impl ClaimBoundWorktree {
    /// Create a worktree on disk for the given `(feature_slug, wp_id)`,
    /// validated against the supplied claim, and update the claim's
    /// reason so the worktree path can be recovered later.
    ///
    /// The claim's *resource* is the canonical
    /// `feat/<feature_slug>/<wp_id>` branch name. The worktree path
    /// itself is recorded as the claim's [`ClaimReason::Branch`] value
    /// â€” i.e. the same branch. To find the on-disk path from a claim,
    /// use [`ClaimBoundWorktree::lookup`].
    pub fn create<S: ClaimStoreBound>(
        repo_root: PathBuf,
        feature_slug: &str,
        wp_id: &str,
        claim: &Claim,
        claim_store: &mut S,
    ) -> Result<PathBuf, DomainError> {
        // (1) Validate the claim.
        Self::validate(claim)?;
        // (2) Create the worktree on disk via the underlying adapter.
        let adapter = GitVcsAdapter::new(repo_root);
        let path = block_on_sync(adapter.create_worktree(feature_slug, wp_id))?;
        // (3) Record the worktree path in the claim's reason by
        // mutating the claim in the store. The store API only exposes
        // `claim()` (which inserts a *new* claim) and `release()` /
        // `heartbeat()`, so we replace the claim by releasing the old
        // id and re-claiming it with the new reason. This is
        // unavoidably racy with concurrent claims, but claim stores
        // are documented as needing external locking, so a `Mutex` at
        // the call site is sufficient.
        let new_claim = Claim {
            id: claim.id.clone(),
            resource: claim.resource.clone(),
            kind: claim.kind,
            agent_id: claim.agent_id.clone(),
            created_at: claim.created_at,
            last_heartbeat: claim.last_heartbeat,
            ttl_seconds: claim.ttl_seconds,
            state: claim.state,
            reason: ClaimReason::Branch(format!("feat/{feature_slug}/{wp_id}:{}", path.display())),
        };
        // Release the old claim id (best-effort; ignore the bool).
        let _ = claim_store.release(&claim.id);
        // Re-claim the resource with the new reason. If a concurrent
        // claim is in flight this may return None, in which case we
        // surface a domain error.
        if claim_store
            .claim(
                &new_claim.id,
                &new_claim.resource,
                new_claim.kind,
                &new_claim.agent_id,
                new_claim.ttl_seconds,
                new_claim.reason.clone(),
            )
            .is_none()
        {
            return Err(DomainError::InvalidClaim(format!(
                "could not re-register claim {} after worktree creation (resource busy)",
                claim.id
            )));
        }
        Ok(path)
    }

    /// Recover the on-disk worktree path that was bound to a claim
    /// by [`create`]. The path is encoded in the claim's
    /// [`ClaimReason::Branch`] value (the bit after the `:`); if the
    /// reason is not a `Branch(...)` or does not carry a path, the
    /// function returns `None`.
    pub fn lookup(claim: &Claim) -> Option<PathBuf> {
        match &claim.reason {
            ClaimReason::Branch(s) => {
                // The recorded value is `feat/<feature_slug>/<wp_id>:<path>`.
                s.split_once(':')
                    .map(|(_, path_str)| PathBuf::from(path_str))
            }
            _ => None,
        }
    }

    /// Validate a claim for worktree creation. Public so the CLI can
    /// pre-check before calling `create`.
    pub fn validate(claim: &Claim) -> Result<(), DomainError> {
        if claim.kind != ClaimKind::Worktree {
            return Err(DomainError::InvalidClaim(format!(
                "expected kind Worktree, got {:?}",
                claim.kind
            )));
        }
        if claim.state != ClaimState::Active {
            return Err(DomainError::InvalidClaim(format!(
                "expected state Active, got {:?}",
                claim.state
            )));
        }
        Ok(())
    }
}

/// Bridge helper so we can use the async `create_worktree` from a sync
/// `create` function without dragging an async runtime into the
/// signature. `create_worktree` does its first meaningful work
/// synchronously (we shell out to `git worktree add`, which is
/// synchronous), so for the claim-bound API we run it on the current
/// thread via [`tokio::task::block_in_place`] when a runtime is
/// present, or just call the future directly otherwise. This keeps
/// the public API sync while staying safe under `tokio::main`.
fn block_on_sync<F: std::future::Future>(fut: F) -> F::Output {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(move || handle.block_on(fut))
    } else {
        // No runtime: build a minimal one.
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime for claim-bound worktree")
            .block_on(fut)
    }
}

/// Re-exported for downstream consumers that want the bounded store
/// surface without depending on `agileplus_triage` directly.
pub use agileplus_triage::claim::ClaimStoreTrait as ClaimStore;

/// Convenience: build a [`Claim`] for a new worktree claim. Pure
/// data â€” does not touch the store.
pub fn make_worktree_claim(
    claim_id: impl Into<String>,
    resource: impl Into<String>,
    agent: impl Into<String>,
    ttl_seconds: i64,
) -> Claim {
    Claim {
        id: claim_id.into(),
        resource: resource.into(),
        kind: ClaimKind::Worktree,
        agent_id: agent.into(),
        created_at: chrono::Utc::now(),
        last_heartbeat: chrono::Utc::now(),
        ttl_seconds,
        state: ClaimState::Active,
        reason: ClaimReason::default(),
    }
}

/// Re-export so the lib.rs `pub mod claim_bound` exposes a usable
/// `ClaimError` without a transitive `agileplus_triage` import in
/// consumer crates.
pub use agileplus_triage::claim::ClaimError as TriageClaimError;

/// Convert a [`ClaimError`] into a [`DomainError`]. Both represent
/// the same shape (NotFound / WrongOwner / WrongState), so the
/// mapping is one-to-one. We cannot implement `From<ClaimError> for
/// DomainError` here because of Rust's orphan rules, so we expose
/// this as a free function for callers that need the mapping.
pub fn map_claim_err(e: ClaimError) -> DomainError {
    match e {
        ClaimError::NotFound(id) => DomainError::NotFound(id),
        ClaimError::WrongOwner { expected, actual } => DomainError::InvalidClaim(format!(
            "wrong owner: expected {expected}, claim held by {actual}"
        )),
        ClaimError::WrongState => DomainError::InvalidClaim("wrong state".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim_bound::{make_worktree_claim, ClaimBoundWorktree};
    use agileplus_triage::claim::ClaimStore;
    use std::process::Command as StdCommand;
    use tempfile::tempdir;

    /// Make a temp git repo with one initial commit on `main`.
    fn make_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();
        StdCommand::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(&path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "t@example.com"])
            .current_dir(&path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "tester"])
            .current_dir(&path)
            .output()
            .unwrap();
        std::fs::write(path.join("README.md"), "hello\n").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(&path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-q", "-m", "init"])
            .current_dir(&path)
            .output()
            .unwrap();
        (dir, path)
    }

    #[test]
    fn validate_rejects_wrong_kind() {
        let c = Claim {
            id: "c1".into(),
            resource: "r".into(),
            kind: ClaimKind::Branch,
            agent_id: "a".into(),
            created_at: chrono::Utc::now(),
            last_heartbeat: chrono::Utc::now(),
            ttl_seconds: 60,
            state: ClaimState::Active,
            reason: ClaimReason::default(),
        };
        let err = ClaimBoundWorktree::validate(&c).unwrap_err();
        match err {
            DomainError::InvalidClaim(msg) => assert!(msg.contains("Worktree")),
            other => panic!("expected InvalidClaim, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_draining_state() {
        let c = Claim {
            id: "c1".into(),
            resource: "r".into(),
            kind: ClaimKind::Worktree,
            agent_id: "a".into(),
            created_at: chrono::Utc::now(),
            last_heartbeat: chrono::Utc::now(),
            ttl_seconds: 60,
            state: ClaimState::Draining,
            reason: ClaimReason::default(),
        };
        let err = ClaimBoundWorktree::validate(&c).unwrap_err();
        match err {
            DomainError::InvalidClaim(msg) => assert!(msg.contains("Active")),
            other => panic!("expected InvalidClaim, got {other:?}"),
        }
    }

    #[test]
    fn make_worktree_claim_defaults_are_correct() {
        let c = make_worktree_claim("c1", "feat/x/wp-1", "agent-a", 60);
        assert_eq!(c.kind, ClaimKind::Worktree);
        assert_eq!(c.state, ClaimState::Active);
        assert_eq!(c.reason, ClaimReason::Manual(String::new()));
    }

    #[test]
    fn create_records_path_in_claim_reason() {
        let (_dir, path) = make_repo();
        let mut store = ClaimStore::new();
        let claim = make_worktree_claim("c1", "feat/login/wp-1", "agent-a", 60);
        store
            .claim(
                &claim.id,
                &claim.resource,
                claim.kind,
                &claim.agent_id,
                claim.ttl_seconds,
                claim.reason.clone(),
            )
            .expect("claim");
        let wt_path = ClaimBoundWorktree::create(path.clone(), "login", "wp-1", &claim, &mut store)
            .expect("create worktree");
        assert!(wt_path.is_dir(), "worktree path should exist");
        // The new claim in the store now carries a Branch reason
        // encoding the worktree path.
        let stored = store.lookup(claim.kind, &claim.resource).expect("lookup");
        match &stored.reason {
            ClaimReason::Branch(s) => {
                assert!(s.contains(&wt_path.display().to_string()));
            }
            other => panic!("expected Branch reason, got {other:?}"),
        }
        // And `lookup` can recover the path.
        assert_eq!(ClaimBoundWorktree::lookup(&stored), Some(wt_path));
    }

    #[test]
    fn create_rejects_branch_kind_claim() {
        let (_dir, path) = make_repo();
        let mut store = ClaimStore::new();
        let claim = Claim {
            id: "c1".into(),
            resource: "r".into(),
            kind: ClaimKind::Branch,
            agent_id: "agent-a".into(),
            created_at: chrono::Utc::now(),
            last_heartbeat: chrono::Utc::now(),
            ttl_seconds: 60,
            state: ClaimState::Active,
            reason: ClaimReason::default(),
        };
        let err =
            ClaimBoundWorktree::create(path, "login", "wp-1", &claim, &mut store).unwrap_err();
        match err {
            DomainError::InvalidClaim(msg) => assert!(msg.contains("Worktree")),
            other => panic!("expected InvalidClaim, got {other:?}"),
        }
    }

    #[test]
    fn lookup_returns_none_for_non_branch_reason() {
        let c = make_worktree_claim("c1", "r", "a", 60);
        assert_eq!(ClaimBoundWorktree::lookup(&c), None);
        let c2 = Claim {
            reason: ClaimReason::TaskRef("wp-1".into()),
            ..c
        };
        assert_eq!(ClaimBoundWorktree::lookup(&c2), None);
    }

    #[test]
    fn map_claim_err_covers_all_variants() {
        let nf = ClaimError::NotFound("abc".into());
        match map_claim_err(nf) {
            DomainError::NotFound(id) => assert_eq!(id, "abc"),
            other => panic!("expected NotFound, got {other:?}"),
        }
        let wo = ClaimError::WrongOwner {
            expected: "a".into(),
            actual: "b".into(),
        };
        let msg = format!("{}", map_claim_err(wo));
        assert!(msg.contains("a") && msg.contains("b"));
        let ws = ClaimError::WrongState;
        let msg = format!("{}", map_claim_err(ws));
        assert!(msg.contains("wrong state"));
    }
}
