// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration test: convoy creation → bead claiming → two-phase commit.

use chrono::Utc;

use agileplus_convoy::bead::Bead;
use agileplus_convoy::coordinator::Coordinator;
use agileplus_convoy::store::ConvoyStore;
use agileplus_convoy::Convoy;
use agileplus_triage::claim::{ClaimKind, ClaimStore};

#[tokio::test]
async fn full_convoy_flow_commit() {
    let mut claim_store = ClaimStore::new();
    let mut convoy_store = ConvoyStore::new();

    // 1. Issue claims
    let claim1 = claim_store
        .claim(
            "c1",
            "repo/a",
            ClaimKind::Repo,
            "agent-1",
            3600,
            Default::default(),
        )
        .expect("claim 1");
    let claim2 = claim_store
        .claim(
            "c2",
            "repo/b",
            ClaimKind::Repo,
            "agent-2",
            3600,
            Default::default(),
        )
        .expect("claim 2");

    // 2. Create convoy and add beads
    let mut convoy = Convoy::new("coordinator-1", Utc::now() + chrono::Duration::hours(1));
    let bead1 = Bead::new(claim1, serde_json::json!({"task": "build"}), "agent-1");
    let bead2 = Bead::new(claim2, serde_json::json!({"task": "test"}), "agent-2");
    convoy.add_bead(bead1);
    convoy.add_bead(bead2);
    convoy.close();

    assert_eq!(convoy.beads.len(), 2);
    assert_eq!(convoy.status, agileplus_convoy::ConvoyStatus::Closed);

    // 3. Beads finish work
    convoy.beads[0].start();
    convoy.beads[1].start();
    convoy.beads[0].complete();
    convoy.beads[1].complete();

    // 4. Two-phase commit
    assert!(Coordinator::prepare(&convoy));
    Coordinator::commit(&mut convoy, &mut claim_store).expect("commit succeeded");

    assert_eq!(convoy.status, agileplus_convoy::ConvoyStatus::Committed);
    assert!(claim_store.lookup(ClaimKind::Repo, "repo/a").is_none());
    assert!(claim_store.lookup(ClaimKind::Repo, "repo/b").is_none());

    // 5. Store round-trip
    convoy_store.add(convoy.clone());
    let loaded = convoy_store.get(convoy.id).expect("convoy in store");
    assert_eq!(loaded.status, agileplus_convoy::ConvoyStatus::Committed);
}

#[tokio::test]
async fn full_convoy_flow_abort() {
    let mut claim_store = ClaimStore::new();

    let claim1 = claim_store
        .claim(
            "c1",
            "repo/a",
            ClaimKind::Repo,
            "agent-1",
            3600,
            Default::default(),
        )
        .expect("claim 1");

    let mut convoy = Convoy::new("coordinator-1", Utc::now() + chrono::Duration::hours(1));
    let bead1 = Bead::new(claim1, serde_json::json!({"task": "build"}), "agent-1");
    convoy.add_bead(bead1);
    convoy.close();

    // One bead fails
    convoy.beads[0].start();
    convoy.beads[0].fail();

    Coordinator::abort(&mut convoy, &mut claim_store).expect("abort succeeded");
    assert_eq!(convoy.status, agileplus_convoy::ConvoyStatus::Aborted);
    assert!(claim_store.lookup(ClaimKind::Repo, "repo/a").is_none());
}
