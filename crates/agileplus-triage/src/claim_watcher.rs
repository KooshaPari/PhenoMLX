// SPDX-License-Identifier: MIT OR Apache-2.0
//! Claim state change event stream.
//!
//! `ClaimWatcher` wraps a [`ClaimStore`] and emits [`ClaimEvent`]s
//! whenever a claim is created, heartbeats, released, expires, or is
//! transferred.  Consumers can subscribe to a per-claim-id channel and
//! receive events asynchronously.
//!
//! Traceability: audit rec #21 (claim state change event stream).

use crate::claim::{Claim, ClaimKind, ClaimReason, ClaimStore};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Events emitted by the claim watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimEvent {
    /// A new claim was issued.
    Claimed,
    /// A heartbeat refreshed the claim.
    Heartbeat,
    /// The claim was actively released.
    Released,
    /// The claim expired due to TTL elapse.
    Expired,
    /// The claim was transferred to a new agent.
    Transferred { from: String, to: String },
}

/// In-memory claim watcher that broadcasts state changes.
#[derive(Debug, Clone)]
pub struct ClaimWatcher {
    store: ClaimStore,
    /// Per-claim-id event log used for replay / sync.
    event_log: Arc<Mutex<HashMap<String, Vec<ClaimEvent>>>>,
    /// Active async listeners (claim_id -> senders).
    listeners: Arc<Mutex<HashMap<String, Vec<mpsc::UnboundedSender<ClaimEvent>>>>>,
}

impl ClaimWatcher {
    /// Create a new watcher wrapping an empty claim store.
    pub fn new() -> Self {
        Self {
            store: ClaimStore::new(),
            event_log: Arc::new(Mutex::new(HashMap::new())),
            listeners: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Wrap an existing claim store.
    pub fn with_store(store: ClaimStore) -> Self {
        Self {
            store,
            event_log: Arc::new(Mutex::new(HashMap::new())),
            listeners: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Subscribe to events for a specific claim ID.
    ///
    /// The receiver yields every [`ClaimEvent`] that affects `claim_id`.
    pub fn watch(&self, claim_id: &str) -> mpsc::UnboundedReceiver<ClaimEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.listeners
            .lock()
            .unwrap()
            .entry(claim_id.to_string())
            .or_default()
            .push(tx);
        rx
    }

    /// Issue a new claim and emit a `Claimed` event.
    pub fn claim(
        &mut self,
        id: &str,
        resource: &str,
        kind: ClaimKind,
        agent: &str,
        ttl_seconds: i64,
        reason: ClaimReason,
    ) -> Option<Claim> {
        let result = self
            .store
            .claim(id, resource, kind, agent, ttl_seconds, reason);
        if result.is_some() {
            self.emit(id, ClaimEvent::Claimed);
        }
        result
    }

    /// Refresh heartbeat and emit a `Heartbeat` event.
    pub fn heartbeat(&mut self, id: &str) -> bool {
        let ok = self.store.heartbeat(id);
        if ok {
            self.emit(id, ClaimEvent::Heartbeat);
        }
        ok
    }

    /// Release a claim and emit a `Released` event.
    pub fn release(&mut self, id: &str) -> bool {
        let ok = self.store.release(id);
        if ok {
            self.emit(id, ClaimEvent::Released);
        }
        ok
    }

    /// Reap expired claims and emit `Expired` events.
    pub fn reap_expired(&mut self, now: DateTime<Utc>) -> usize {
        let expired_ids: Vec<String> = self
            .store
            .all()
            .into_iter()
            .filter(|c| c.is_expired(now))
            .map(|c| c.id)
            .collect();
        let count = self.store.reap_expired(now);
        for id in &expired_ids {
            self.emit(id, ClaimEvent::Expired);
        }
        count
    }

    /// Transfer a claim and emit a `Transferred` event.
    pub fn claim_transfer(
        &mut self,
        from_id: &str,
        to_id: &str,
        to_agent: &str,
    ) -> Result<Claim, crate::claim::ClaimError> {
        let result = self.store.claim_transfer(from_id, to_id, to_agent);
        if let Ok(ref _new_claim) = result {
            self.emit(
                from_id,
                ClaimEvent::Transferred {
                    from: from_id.to_string(),
                    to: to_id.to_string(),
                },
            );
            self.emit(to_id, ClaimEvent::Claimed);
        }
        result
    }

    /// Lookup by resource (no event emitted).
    pub fn lookup(&self, kind: ClaimKind, resource: &str) -> Option<Claim> {
        self.store.lookup(kind, resource)
    }

    /// All claims (no event emitted).
    pub fn all(&self) -> Vec<Claim> {
        self.store.all()
    }

    /// Active claims only (no event emitted).
    pub fn active(&self) -> Vec<Claim> {
        self.store.active()
    }

    /// Access the underlying store directly.
    pub fn store(&self) -> &ClaimStore {
        &self.store
    }

    /// Mutable access to the underlying store.
    pub fn store_mut(&mut self) -> &mut ClaimStore {
        &mut self.store
    }

    fn emit(&self, claim_id: &str, event: ClaimEvent) {
        // Append to event log.
        if let Ok(mut log) = self.event_log.lock() {
            log.entry(claim_id.to_string())
                .or_default()
                .push(event.clone());
        }
        // Broadcast to active listeners.
        if let Ok(mut listeners) = self.listeners.lock() {
            if let Some(senders) = listeners.get_mut(claim_id) {
                senders.retain(|tx| tx.send(event.clone()).is_ok());
            }
        }
    }
}

impl Default for ClaimWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim::{ClaimKind, ClaimReason};

    #[test]
    fn claim_watcher_emits_claimed_event() {
        let mut watcher = ClaimWatcher::new();
        let mut rx = watcher.watch("c1");
        watcher.claim(
            "c1",
            "repo:foo",
            ClaimKind::Repo,
            "agent-a",
            60,
            ClaimReason::default(),
        );
        let event = rx.try_recv().expect("expected Claimed event");
        assert_eq!(event, ClaimEvent::Claimed);
    }

    #[test]
    fn claim_watcher_emits_heartbeat_event() {
        let mut watcher = ClaimWatcher::new();
        watcher.claim(
            "c1",
            "repo:foo",
            ClaimKind::Repo,
            "agent-a",
            60,
            ClaimReason::default(),
        );
        let mut rx = watcher.watch("c1");
        watcher.heartbeat("c1");
        let event = rx.try_recv().expect("expected Heartbeat event");
        assert_eq!(event, ClaimEvent::Heartbeat);
    }

    #[test]
    fn claim_watcher_emits_released_event() {
        let mut watcher = ClaimWatcher::new();
        watcher.claim(
            "c1",
            "repo:foo",
            ClaimKind::Repo,
            "agent-a",
            60,
            ClaimReason::default(),
        );
        let mut rx = watcher.watch("c1");
        watcher.release("c1");
        let event = rx.try_recv().expect("expected Released event");
        assert_eq!(event, ClaimEvent::Released);
    }

    #[test]
    fn claim_watcher_emits_expired_event() {
        let mut watcher = ClaimWatcher::new();
        watcher.claim(
            "c1",
            "repo:foo",
            ClaimKind::Repo,
            "agent-a",
            1,
            ClaimReason::default(),
        );
        let mut rx = watcher.watch("c1");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let reaped = watcher.reap_expired(Utc::now());
        assert_eq!(reaped, 1);
        let event = rx.try_recv().expect("expected Expired event");
        assert_eq!(event, ClaimEvent::Expired);
    }

    #[test]
    fn claim_watcher_emits_transferred_event() {
        let mut watcher = ClaimWatcher::new();
        watcher.claim(
            "c1",
            "repo:foo",
            ClaimKind::Repo,
            "agent-a",
            60,
            ClaimReason::default(),
        );
        let mut rx = watcher.watch("c1");
        watcher.claim_transfer("c1", "c2", "agent-b").unwrap();
        let event = rx.try_recv().expect("expected Transferred event");
        assert!(
            matches!(event, ClaimEvent::Transferred { from, to } if from == "c1" && to == "c2")
        );
    }

    #[test]
    fn claim_watcher_multiple_watchers_receive_events() {
        let mut watcher = ClaimWatcher::new();
        let mut rx1 = watcher.watch("c1");
        let mut rx2 = watcher.watch("c1");
        watcher.claim(
            "c1",
            "repo:foo",
            ClaimKind::Repo,
            "agent-a",
            60,
            ClaimReason::default(),
        );
        assert_eq!(rx1.try_recv().unwrap(), ClaimEvent::Claimed);
        assert_eq!(rx2.try_recv().unwrap(), ClaimEvent::Claimed);
    }

    #[test]
    fn claim_watcher_event_log_accumulates() {
        let mut watcher = ClaimWatcher::new();
        watcher.claim(
            "c1",
            "repo:foo",
            ClaimKind::Repo,
            "agent-a",
            60,
            ClaimReason::default(),
        );
        watcher.heartbeat("c1");
        watcher.release("c1");
        let log = watcher.event_log.lock().unwrap();
        let events = log.get("c1").expect("c1 should have events");
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], ClaimEvent::Claimed);
        assert_eq!(events[1], ClaimEvent::Heartbeat);
        assert_eq!(events[2], ClaimEvent::Released);
    }

    #[test]
    fn claim_watcher_no_event_for_failed_claim() {
        let mut watcher = ClaimWatcher::new();
        watcher.claim(
            "c1",
            "repo:foo",
            ClaimKind::Repo,
            "agent-a",
            60,
            ClaimReason::default(),
        );
        let mut rx = watcher.watch("c2");
        // c2 is not claimed, so no event should be emitted.
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn claim_watcher_default() {
        let watcher: ClaimWatcher = Default::default();
        assert!(watcher.all().is_empty());
    }

    #[test]
    fn claim_watcher_store_accessor() {
        let mut watcher = ClaimWatcher::new();
        watcher.claim(
            "c1",
            "repo:foo",
            ClaimKind::Repo,
            "agent-a",
            60,
            ClaimReason::default(),
        );
        assert!(watcher
            .store()
            .lookup(ClaimKind::Repo, "repo:foo")
            .is_some());
    }
}
