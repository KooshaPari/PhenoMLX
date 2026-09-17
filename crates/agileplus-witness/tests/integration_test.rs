// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration test: convoy → bead claiming → witness verification → two-phase commit.

use chrono::Utc;

use agileplus_convoy::bead::Bead;
use agileplus_convoy::coordinator::Coordinator;
use agileplus_convoy::Convoy;
use agileplus_triage::claim::{ClaimKind, ClaimStore};
use agileplus_witness::evidence::Evidence;
use agileplus_witness::verdict::Verdict;
use agileplus_witness::verdict::VerdictEngine;
use agileplus_witness::Witness;

#[tokio::test]
async fn full_witness_flow_pass() {
    let mut claim_store = ClaimStore::new();

    // 1. Claims and convoy
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

    let mut convoy = Convoy::new("coordinator-1", Utc::now() + chrono::Duration::hours(1));
    let bead1 = Bead::new(claim1, serde_json::json!({"task": "build"}), "agent-1");
    let bead2 = Bead::new(claim2, serde_json::json!({"task": "test"}), "agent-2");
    convoy.add_bead(bead1);
    convoy.add_bead(bead2);
    convoy.close();

    // 2. Witnesses with evidence
    let witnesses = vec![
        Witness::new(
            "w1",
            convoy.beads[0].id,
            Verdict::Pass,
            vec![
                Evidence::TestResult {
                    suite: "unit".to_string(),
                    passed: 42,
                    failed: 0,
                },
                Evidence::Diff {
                    from: "main".to_string(),
                    to: "feat/x".to_string(),
                    lines_changed: 12,
                },
            ],
            "agent-1",
        ),
        Witness::new(
            "w2",
            convoy.beads[0].id,
            Verdict::Pass,
            vec![Evidence::CodeReview {
                reviewer: "alice".to_string(),
                notes: "lgtm".to_string(),
            }],
            "agent-2",
        ),
        Witness::new(
            "w3",
            convoy.beads[1].id,
            Verdict::Pass,
            vec![Evidence::TestResult {
                suite: "integration".to_string(),
                passed: 10,
                failed: 0,
            }],
            "agent-3",
        ),
    ];

    // 3. Verdict engine evaluates
    let engine = VerdictEngine;
    let changed = engine.evaluate(&mut convoy, &witnesses).expect("evaluate");
    assert_eq!(changed.len(), 2);
    assert!(convoy
        .beads
        .iter()
        .all(|b| b.state == agileplus_convoy::bead::BeadState::Completed));

    // 4. Two-phase commit
    assert!(Coordinator::prepare(&convoy));
    Coordinator::commit(&mut convoy, &mut claim_store).expect("commit");
    assert_eq!(convoy.status, agileplus_convoy::ConvoyStatus::Committed);
}

#[tokio::test]
async fn full_witness_flow_fail() {
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

    let witnesses = vec![
        Witness::new(
            "w1",
            convoy.beads[0].id,
            Verdict::Fail,
            vec![Evidence::TestResult {
                suite: "unit".to_string(),
                passed: 0,
                failed: 5,
            }],
            "agent-1",
        ),
        Witness::new(
            "w2",
            convoy.beads[0].id,
            Verdict::Abstain,
            vec![],
            "agent-2",
        ),
    ];

    let engine = VerdictEngine;
    engine.evaluate(&mut convoy, &witnesses).expect("evaluate");
    assert_eq!(
        convoy.beads[0].state,
        agileplus_convoy::bead::BeadState::Failed
    );

    Coordinator::abort(&mut convoy, &mut claim_store).expect("abort");
    assert_eq!(convoy.status, agileplus_convoy::ConvoyStatus::Aborted);
}
