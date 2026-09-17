// SPDX-License-Identifier: MIT OR Apache-2.0
//! Event replication over NATS between AgilePlus peers.
//!
//! This module now also hosts the in-memory delta replication protocol used by
//! the peer-to-peer work-tracking state machine. The transport helpers remain
//! for existing device sync callers, while the new state types are independent
//! of network wiring.
//!
//! Traceability: WP16 / T098

use std::collections::BTreeMap;

use agileplus_domain::domain::{epic::Epic, project::Project, story::Story, user::User};
use async_nats::jetstream;
use chrono::{DateTime, Utc};
use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

use crate::discovery::PeerInfo;
use crate::error::SyncError;
use crate::vector_clock::{ClockRelation, SyncVector};

// ── In-memory replication model ──────────────────────────────────────────────

/// Domain aggregate kinds carried by the replication protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DomainKind {
    User,
    Project,
    Epic,
    Story,
}

/// Stable identifier for one replicated aggregate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ItemKey {
    pub kind: DomainKind,
    pub id: i64,
}

/// In-memory representation of a replicated aggregate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "item")]
pub enum DomainItem {
    User(User),
    Project(Project),
    Epic(Epic),
    Story(Story),
}

impl DomainItem {
    pub fn key(&self) -> ItemKey {
        match self {
            DomainItem::User(item) => ItemKey {
                kind: DomainKind::User,
                id: item.id,
            },
            DomainItem::Project(item) => ItemKey {
                kind: DomainKind::Project,
                id: item.id,
            },
            DomainItem::Epic(item) => ItemKey {
                kind: DomainKind::Epic,
                id: item.id,
            },
            DomainItem::Story(item) => ItemKey {
                kind: DomainKind::Story,
                id: item.id,
            },
        }
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        match self {
            DomainItem::User(item) => item.updated_at,
            DomainItem::Project(item) => item.updated_at,
            DomainItem::Epic(item) => item.updated_at,
            DomainItem::Story(item) => item.updated_at,
        }
    }
}

/// Snapshot of one aggregate version and its causal history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedItem {
    pub item: DomainItem,
    pub clock: SyncVector,
}

impl VersionedItem {
    pub fn new(item: DomainItem, clock: SyncVector) -> Self {
        Self { item, clock }
    }
}

/// Item payload sent to another peer during a delta exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    pub key: ItemKey,
    pub item: DomainItem,
    pub clock: SyncVector,
}

impl Delta {
    pub fn new(item: DomainItem, clock: SyncVector) -> Self {
        let key = item.key();
        Self { key, item, clock }
    }
}

/// Winner chosen for a concurrent conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictWinner {
    Local,
    Incoming,
}

/// Details of a concurrent conflict detected during merge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub key: ItemKey,
    pub local: DomainItem,
    pub incoming: DomainItem,
    pub local_clock: SyncVector,
    pub incoming_clock: SyncVector,
    pub winner: ConflictWinner,
    pub resolved: DomainItem,
}

/// Result of applying a batch of incoming deltas.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MergeOutcome {
    pub applied: usize,
    pub skipped: usize,
    pub conflicts: Vec<Conflict>,
}

/// Per-item clock index keyed by aggregate identity.
pub type ItemClockMap = BTreeMap<ItemKey, SyncVector>;

/// In-memory replication state for a peer.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ReplicationState {
    pub items: BTreeMap<ItemKey, VersionedItem>,
}

impl ReplicationState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or update an item locally and advance the actor component of its clock.
    pub fn upsert_local(&mut self, replica_id: &str, item: DomainItem) -> ItemKey {
        let key = item.key();
        let mut clock = self
            .items
            .get(&key)
            .map(|existing| existing.clock.clone())
            .unwrap_or_else(|| SyncVector::new(replica_id));
        bump_replica(&mut clock, replica_id);
        self.items
            .insert(key.clone(), VersionedItem::new(item, clock));
        key
    }

    /// Return the current causal clock for every known item.
    pub fn clocks(&self) -> ItemClockMap {
        self.items
            .iter()
            .map(|(key, versioned)| (key.clone(), versioned.clock.clone()))
            .collect()
    }

    /// Compute the deltas the remote peer has not yet seen.
    ///
    /// Items are sent when our clock dominates the remote clock, or when the
    /// clocks are concurrent and we need to expose our branch to the peer.
    pub fn diff_against(&self, remote_clock: &ItemClockMap) -> Vec<Delta> {
        let mut deltas = self
            .items
            .iter()
            .filter_map(|(key, local)| {
                let should_send = match remote_clock.get(key) {
                    None => true,
                    Some(remote) => matches!(
                        local.clock.compare(remote),
                        ClockRelation::Greater | ClockRelation::Concurrent
                    ),
                };

                should_send.then(|| Delta {
                    key: key.clone(),
                    item: local.item.clone(),
                    clock: local.clock.clone(),
                })
            })
            .collect::<Vec<_>>();
        deltas.sort_by(|a, b| a.key.cmp(&b.key));
        deltas
    }

    /// Apply incoming deltas using vector-clock comparison.
    pub fn apply_deltas<I>(&mut self, deltas: I) -> MergeOutcome
    where
        I: IntoIterator<Item = Delta>,
    {
        let mut outcome = MergeOutcome::default();

        for delta in deltas {
            match self.items.get_mut(&delta.key) {
                None => {
                    self.items.insert(
                        delta.key.clone(),
                        VersionedItem::new(delta.item.clone(), delta.clock.clone()),
                    );
                    outcome.applied += 1;
                }
                Some(local) => {
                    let relation = local.clock.compare(&delta.clock);
                    match relation {
                        ClockRelation::Equal => {
                            outcome.skipped += 1;
                        }
                        ClockRelation::Greater => {
                            // Local already dominates the incoming version.
                            outcome.skipped += 1;
                        }
                        ClockRelation::Less => {
                            local.item = delta.item.clone();
                            local.clock = delta.clock.clone();
                            outcome.applied += 1;
                        }
                        ClockRelation::Concurrent => {
                            let local_snapshot = local.item.clone();
                            let local_clock = local.clock.clone();
                            let incoming_clock = delta.clock.clone();

                            let winner = choose_lww_winner(&local_snapshot, &delta.item);
                            let resolved = match winner {
                                ConflictWinner::Local => local_snapshot.clone(),
                                ConflictWinner::Incoming => delta.item.clone(),
                            };

                            local.clock = merged_clock(&local_clock, &incoming_clock);
                            if matches!(winner, ConflictWinner::Incoming) {
                                local.item = delta.item.clone();
                            }

                            outcome.conflicts.push(Conflict {
                                key: delta.key.clone(),
                                local: local_snapshot,
                                incoming: delta.item.clone(),
                                local_clock,
                                incoming_clock,
                                winner,
                                resolved: resolved.clone(),
                            });
                            outcome.applied += 1;
                        }
                    }
                }
            }
        }

        outcome
    }
}

fn merged_clock(left: &SyncVector, right: &SyncVector) -> SyncVector {
    let mut merged = left.clone();
    merged.merge(right);
    merged
}

fn bump_replica(clock: &mut SyncVector, replica_id: &str) {
    let entry = clock.get(replica_id, CLOCK_ENTRY_ID);
    clock.advance(replica_id, CLOCK_ENTRY_ID, entry + 1);
}

fn choose_lww_winner(local: &DomainItem, incoming: &DomainItem) -> ConflictWinner {
    match local.updated_at().cmp(&incoming.updated_at()) {
        std::cmp::Ordering::Less => ConflictWinner::Incoming,
        std::cmp::Ordering::Greater => ConflictWinner::Local,
        std::cmp::Ordering::Equal => {
            let local_json = serde_json::to_string(local).unwrap_or_default();
            let incoming_json = serde_json::to_string(incoming).unwrap_or_default();
            if incoming_json >= local_json {
                ConflictWinner::Incoming
            } else {
                ConflictWinner::Local
            }
        }
    }
}

const CLOCK_ENTRY_ID: &str = "__clock__";

// ── NATS subject helpers ──────────────────────────────────────────────────────

/// Subject to which a device publishes events destined for `target_device_id`.
pub fn device_subject(target_device_id: &str) -> String {
    format!("agileplus.sync.device.{target_device_id}")
}

// ── Wire format ───────────────────────────────────────────────────────────────

/// Serialisable wrapper around a batch of events for over-the-wire transfer.
#[derive(Debug, Serialize, Deserialize)]
pub struct EventBatch {
    pub sender_device_id: String,
    pub events: Vec<agileplus_domain::domain::event::Event>,
}

// ── Result type ───────────────────────────────────────────────────────────────

/// Outcome of a single replication attempt with one peer.
#[derive(Debug, Default)]
pub struct ReplicationResult {
    pub events_sent: usize,
    pub events_received: usize,
}

// ── Core replication logic ────────────────────────────────────────────────────

/// Replicate `events` to `peer` and collect any events the peer sends back.
///
/// Connection is attempted with exponential backoff (3 retries: 1 s, 2 s, 4 s).
pub async fn replicate_events(
    local_device_id: &str,
    peer: &PeerInfo,
    events: Vec<agileplus_domain::domain::event::Event>,
) -> Result<ReplicationResult, SyncError> {
    let url = format!("nats://{}:4222", peer.tailscale_ip);
    let client = connect_with_retry(&url, &peer.device_id).await?;
    let js = jetstream::new(client);

    // Ensure the peer's stream exists (or create it).
    let peer_stream_name = format!(
        "AGILEPLUS_SYNC_{}",
        peer.device_id.replace('-', "_").to_uppercase()
    );
    let peer_subject = device_subject(&peer.device_id);
    let _ = js
        .get_or_create_stream(jetstream::stream::Config {
            name: peer_stream_name.clone(),
            subjects: vec![peer_subject.clone()],
            ..Default::default()
        })
        .await;

    // Publish events to the peer's subject.
    let events_sent = events.len();
    let batch = EventBatch {
        sender_device_id: local_device_id.to_string(),
        events,
    };
    let payload = serde_json::to_vec(&batch)?;

    js.publish(peer_subject.clone(), payload.into())
        .await
        .map_err(|e| SyncError::PublishFailed(e.to_string()))?
        .await
        .map_err(|e| SyncError::PublishFailed(e.to_string()))?;

    info!(
        "Sent {events_sent} events to peer {} on {url}",
        peer.device_id
    );

    // Attempt to drain any events the peer already published to our subject.
    let local_subject = device_subject(local_device_id);
    let local_stream_name = format!(
        "AGILEPLUS_SYNC_{}",
        local_device_id.replace('-', "_").to_uppercase()
    );
    let _ = js
        .get_or_create_stream(jetstream::stream::Config {
            name: local_stream_name.clone(),
            subjects: vec![local_subject.clone()],
            ..Default::default()
        })
        .await;

    let events_received = drain_pending(&js, &local_stream_name, &local_subject).await;

    Ok(ReplicationResult {
        events_sent,
        events_received,
    })
}

/// Connect to a NATS server with up to 3 retries (1 s / 2 s / 4 s).
async fn connect_with_retry(url: &str, peer_id: &str) -> Result<async_nats::Client, SyncError> {
    let delays = [
        Duration::from_secs(1),
        Duration::from_secs(2),
        Duration::from_secs(4),
    ];
    let mut last_err = SyncError::ConnectionFailed {
        peer_id: peer_id.to_string(),
        reason: "no attempts made".to_string(),
    };

    for (attempt, delay) in delays.iter().enumerate() {
        match async_nats::connect(url).await {
            Ok(c) => {
                debug!("Connected to NATS at {url} on attempt {}", attempt + 1);
                return Ok(c);
            }
            Err(e) => {
                warn!("NATS connect attempt {} to {url} failed: {e}", attempt + 1);
                last_err = SyncError::ConnectionFailed {
                    peer_id: peer_id.to_string(),
                    reason: e.to_string(),
                };
                sleep(*delay).await;
            }
        }
    }
    error!("All NATS connect attempts to {url} failed for peer {peer_id}");
    Err(last_err)
}

/// Pull and count any messages already queued in `stream` / `subject`.
async fn drain_pending(js: &jetstream::Context, stream_name: &str, subject: &str) -> usize {
    use tokio::time::timeout;

    let consumer_cfg = jetstream::consumer::pull::Config {
        filter_subject: subject.to_string(),
        deliver_policy: jetstream::consumer::DeliverPolicy::All,
        ..Default::default()
    };

    let stream = match js.get_stream(stream_name).await {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let consumer = match stream.create_consumer(consumer_cfg).await {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let mut count = 0usize;
    loop {
        let batch = match timeout(
            Duration::from_millis(250),
            consumer.fetch().max_messages(50).messages(),
        )
        .await
        {
            Ok(Ok(b)) => b,
            _ => break,
        };

        let msgs: Vec<_> = batch
            .take_until(tokio::time::sleep(Duration::from_millis(100)))
            .collect()
            .await;

        if msgs.is_empty() {
            break;
        }

        for msg in msgs.into_iter().flatten() {
            count += 1;
            let _ = msg.ack().await;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    use agileplus_domain::domain::{
        epic::{Epic, EpicStatus},
        project::Project,
        story::{Story, StoryStatus},
        user::{User, UserRole, UserStatus},
    };

    fn clock(replica: &str, counter: u64) -> SyncVector {
        let mut clock = SyncVector::new(replica);
        clock.advance(replica, CLOCK_ENTRY_ID, counter);
        clock
    }

    fn sample_user(id: i64, updated_at: DateTime<Utc>) -> User {
        User {
            id,
            display_name: format!("User {id}"),
            email: format!("user{id}@example.com"),
            role: UserRole::Member,
            status: UserStatus::Active,
            avatar_url: None,
            github_login: None,
            created_at: updated_at,
            updated_at,
        }
    }

    fn sample_project(id: i64, updated_at: DateTime<Utc>) -> Project {
        Project {
            id,
            slug: format!("project-{id}"),
            name: format!("Project {id}"),
            description: None,
            created_at: updated_at,
            updated_at,
        }
    }

    fn sample_epic(id: i64, project_id: i64, updated_at: DateTime<Utc>) -> Epic {
        Epic {
            id,
            project_id,
            title: format!("Epic {id}"),
            description: None,
            status: EpicStatus::Active,
            owner_id: None,
            created_at: updated_at,
            updated_at,
        }
    }

    fn sample_story(id: i64, epic_id: i64, project_id: i64, updated_at: DateTime<Utc>) -> Story {
        Story {
            id,
            epic_id,
            project_id,
            title: format!("Story {id}"),
            description: None,
            status: StoryStatus::Todo,
            points: Some(3),
            assignee_id: None,
            created_at: updated_at,
            updated_at,
        }
    }

    #[test]
    fn fresh_peer_pulls_all_items() {
        let now = Utc::now();
        let mut state = ReplicationState::new();
        state.items.insert(
            ItemKey {
                kind: DomainKind::User,
                id: 1,
            },
            VersionedItem::new(DomainItem::User(sample_user(1, now)), clock("peer-a", 1)),
        );
        state.items.insert(
            ItemKey {
                kind: DomainKind::Project,
                id: 2,
            },
            VersionedItem::new(
                DomainItem::Project(sample_project(2, now)),
                clock("peer-a", 2),
            ),
        );

        let deltas = state.diff_against(&ItemClockMap::new());
        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0].key.kind, DomainKind::User);
        assert_eq!(deltas[1].key.kind, DomainKind::Project);
    }

    #[test]
    fn no_op_when_clocks_equal() {
        let now = Utc::now();
        let mut state = ReplicationState::new();
        let key = ItemKey {
            kind: DomainKind::Epic,
            id: 7,
        };
        let item = DomainItem::Epic(sample_epic(7, 99, now));
        let version = VersionedItem::new(item.clone(), clock("peer-a", 3));
        state.items.insert(key.clone(), version.clone());

        let outcome = state.apply_deltas(vec![Delta::new(item, clock("peer-a", 3))]);
        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.skipped, 1);
        assert!(outcome.conflicts.is_empty());
        assert_eq!(
            state.items.get(&key).unwrap().clock.compare(&version.clock),
            ClockRelation::Equal
        );
    }

    #[test]
    fn causal_update_applied() {
        let base = Utc::now();
        let mut state = ReplicationState::new();
        let key = ItemKey {
            kind: DomainKind::Story,
            id: 9,
        };
        state.items.insert(
            key.clone(),
            VersionedItem::new(
                DomainItem::Story(sample_story(9, 1, 2, base)),
                clock("peer-a", 1),
            ),
        );

        let newer = DomainItem::Story(sample_story(9, 1, 2, base + chrono::Duration::seconds(10)));
        let outcome = state.apply_deltas(vec![Delta::new(newer.clone(), clock("peer-a", 2))]);
        assert_eq!(outcome.applied, 1);
        assert!(outcome.conflicts.is_empty());
        match &state.items.get(&key).unwrap().item {
            DomainItem::Story(story) => {
                assert_eq!(story.updated_at, base + chrono::Duration::seconds(10))
            }
            _ => panic!("expected story"),
        }
    }

    #[test]
    fn concurrent_edits_flagged_as_conflict() {
        let base = Utc::now();
        let mut state = ReplicationState::new();
        let key = ItemKey {
            kind: DomainKind::User,
            id: 11,
        };
        state.items.insert(
            key.clone(),
            VersionedItem::new(DomainItem::User(sample_user(11, base)), clock("peer-a", 1)),
        );

        let mut incoming_user = sample_user(11, base + chrono::Duration::seconds(5));
        incoming_user.display_name = "Remote User".to_string();
        let incoming = DomainItem::User(incoming_user);
        let outcome = state.apply_deltas(vec![Delta::new(incoming.clone(), clock("peer-b", 1))]);
        assert_eq!(outcome.conflicts.len(), 1);
        assert_eq!(outcome.applied, 1);
        assert_eq!(outcome.conflicts[0].key, key);
        assert_eq!(outcome.conflicts[0].winner, ConflictWinner::Incoming);
        match &state.items.get(&key).unwrap().item {
            DomainItem::User(user) => assert_eq!(user.display_name, "Remote User"),
            _ => panic!("expected user"),
        }
    }

    #[test]
    fn delta_diff_correctness() {
        let base = Utc::now();
        let mut state = ReplicationState::new();
        let user_key = ItemKey {
            kind: DomainKind::User,
            id: 1,
        };
        let project_key = ItemKey {
            kind: DomainKind::Project,
            id: 2,
        };
        let epic_key = ItemKey {
            kind: DomainKind::Epic,
            id: 3,
        };

        state.items.insert(
            user_key.clone(),
            VersionedItem::new(DomainItem::User(sample_user(1, base)), clock("peer-a", 3)),
        );
        state.items.insert(
            project_key.clone(),
            VersionedItem::new(
                DomainItem::Project(sample_project(2, base)),
                clock("peer-a", 2),
            ),
        );
        state.items.insert(
            epic_key.clone(),
            VersionedItem::new(
                DomainItem::Epic(sample_epic(3, 2, base)),
                clock("peer-a", 1),
            ),
        );

        let mut remote_clock = ItemClockMap::new();
        remote_clock.insert(user_key.clone(), clock("peer-a", 3));
        remote_clock.insert(project_key.clone(), clock("peer-a", 1));
        remote_clock.insert(epic_key.clone(), clock("peer-b", 1));

        let deltas = state.diff_against(&remote_clock);
        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0].key, project_key);
        assert_eq!(deltas[1].key, epic_key);
    }

    #[test]
    fn concurrent_local_wins_still_merges_clock() {
        let base = Utc::now();
        let mut state = ReplicationState::new();
        let key = ItemKey {
            kind: DomainKind::Project,
            id: 44,
        };
        state.items.insert(
            key.clone(),
            VersionedItem::new(
                DomainItem::Project(sample_project(44, base + chrono::Duration::seconds(2))),
                clock("peer-a", 2),
            ),
        );

        let incoming = DomainItem::Project(sample_project(44, base));
        let outcome = state.apply_deltas(vec![Delta::new(incoming, clock("peer-b", 1))]);
        assert_eq!(outcome.conflicts.len(), 1);
        assert_eq!(outcome.conflicts[0].winner, ConflictWinner::Local);
        let merged_clock = &state.items.get(&key).unwrap().clock;
        assert_eq!(merged_clock.get("peer-a", CLOCK_ENTRY_ID), 2);
        assert_eq!(merged_clock.get("peer-b", CLOCK_ENTRY_ID), 1);
    }

    #[test]
    fn device_subject_format() {
        let s = device_subject("device-abc-123");
        assert_eq!(s, "agileplus.sync.device.device-abc-123");
    }

    #[test]
    fn event_batch_roundtrip() {
        use agileplus_domain::domain::event::Event;
        let batch = EventBatch {
            sender_device_id: "dev-1".to_string(),
            events: vec![Event::new(
                "Feature",
                42,
                "created",
                serde_json::json!({}),
                "test",
            )],
        };
        let json = serde_json::to_string(&batch).unwrap();
        let back: EventBatch = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sender_device_id, "dev-1");
        assert_eq!(back.events.len(), 1);
    }
}
