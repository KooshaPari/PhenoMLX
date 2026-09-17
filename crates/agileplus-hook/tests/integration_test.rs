// SPDX-License-Identifier: MIT OR Apache-2.0
//! Integration test: hook registration → claim event → dispatch.

use agileplus_hook::dispatch::{DispatchResult, HookDispatcher};
use agileplus_hook::registry::HookRegistry;
use agileplus_hook::{Hook, HookAction, HookTrigger};
use agileplus_triage::claim::{Claim, ClaimKind, ClaimState};

#[tokio::test]
async fn hook_dispatches_on_claim() {
    let mut registry = HookRegistry::new();

    // 1. Register a script hook
    let hook = Hook::new(
        "h1",
        HookTrigger::OnClaim,
        HookAction::Script {
            command: "echo hello".to_string(),
        },
        Some("repo/.*".to_string()),
    );
    registry.register(hook);

    // 2. Build a dispatcher
    let dispatcher = HookDispatcher::new(registry);

    // 3. Create a claim
    let claim = Claim {
        id: "c1".to_string(),
        resource: "repo/a".to_string(),
        kind: ClaimKind::Repo,
        agent_id: "agent-1".to_string(),
        created_at: chrono::Utc::now(),
        last_heartbeat: chrono::Utc::now(),
        ttl_seconds: 3600,
        state: ClaimState::Active,
        reason: Default::default(),
    };

    // 4. Dispatch
    let results = dispatcher
        .dispatch_claim(&claim, HookTrigger::OnClaim)
        .await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "h1");
    assert_eq!(results[0].1, DispatchResult::Dispatched);
}

#[tokio::test]
async fn hook_filters_by_regex() {
    let mut registry = HookRegistry::new();
    registry.register(Hook::new(
        "h1",
        HookTrigger::OnClaim,
        HookAction::Message {
            topic: "claims".to_string(),
        },
        Some("branch/.*".to_string()),
    ));

    let dispatcher = HookDispatcher::new(registry);
    let claim = Claim {
        id: "c1".to_string(),
        resource: "repo/a".to_string(),
        kind: ClaimKind::Repo,
        agent_id: "agent-1".to_string(),
        created_at: chrono::Utc::now(),
        last_heartbeat: chrono::Utc::now(),
        ttl_seconds: 3600,
        state: ClaimState::Active,
        reason: Default::default(),
    };

    let results = dispatcher
        .dispatch_claim(&claim, HookTrigger::OnClaim)
        .await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, DispatchResult::Filtered);
}

#[tokio::test]
async fn hook_unregister_works() {
    let mut registry = HookRegistry::new();
    registry.register(Hook::new(
        "h1",
        HookTrigger::OnRelease,
        HookAction::Webhook {
            url: "http://localhost:9999/hook".to_string(),
        },
        None,
    ));
    assert_eq!(registry.len(), 1);
    registry.unregister("h1");
    assert!(registry.is_empty());
}
