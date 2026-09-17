// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hook dispatcher — matches claim events to registered hooks.

use std::collections::HashMap;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use regex::Regex;
use tracing::{info, warn};

use agileplus_events::domain_event::{DomainEvent, EventEnvelope};
use agileplus_triage::claim::Claim;

use crate::registry::HookRegistry;
use crate::{HookAction, HookTrigger};

/// Process-wide cache of compiled hook condition regexes.
///
/// Hook patterns are typically configured once at startup and then matched
/// against many claim events; compiling once avoids per-event overhead.
/// Each compiled regex is leaked into a `'static` reference because hook
/// patterns live for the entire process lifetime in practice.
fn regex_cache() -> &'static Mutex<HashMap<String, &'static Regex>> {
    static CACHE: OnceLock<Mutex<HashMap<String, &'static Regex>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Result of a single hook dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchResult {
    Dispatched,
    Filtered,
    Failed(String),
}

/// Dispatches claim events to matching hooks.
#[derive(Debug, Clone)]
pub struct HookDispatcher {
    registry: HookRegistry,
    client: reqwest::Client,
}

impl HookDispatcher {
    /// Create a new dispatcher with the given registry.
    pub fn new(registry: HookRegistry) -> Self {
        Self {
            registry,
            client: reqwest::Client::new(),
        }
    }

    /// Dispatch a claim event against all registered hooks.
    ///
    /// Returns a vector of `(hook_id, result)` pairs.
    pub async fn dispatch_claim(
        &self,
        claim: &Claim,
        trigger: HookTrigger,
    ) -> Vec<(String, DispatchResult)> {
        let mut results = Vec::new();
        for hook in self.registry.list() {
            if hook.trigger != trigger {
                continue;
            }
            // apply regex condition if present (cached per-pattern compile)
            if let Some(ref pattern) = hook.condition {
                let pat = pattern.as_str();
                let re: &'static Regex = {
                    let cache = regex_cache().lock().expect("regex cache poisoned");
                    if let Some(re) = cache.get(pat).copied() {
                        re
                    } else {
                        drop(cache);
                        match Regex::new(pat) {
                            Ok(re) => {
                                let leaked: &'static Regex = Box::leak(Box::new(re));
                                let mut cache =
                                    regex_cache().lock().expect("regex cache poisoned");
                                cache.insert(pat.to_owned(), leaked);
                                leaked
                            }
                            Err(e) => {
                                results.push((
                                    hook.id.clone(),
                                    DispatchResult::Failed(format!("bad regex: {e}")),
                                ));
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        results.push((
                            hook.id.clone(),
                            DispatchResult::Failed(format!("bad regex: {e}")),
                        ));
                        continue;
                    }
                }
            }
            let res = match &hook.action {
                HookAction::Webhook { url } => self.dispatch_webhook(url.as_str(), claim).await,
                HookAction::Message { topic } => self.dispatch_message(topic.as_str(), claim).await,
                HookAction::Script { command } => {
                    self.dispatch_script(command.as_str(), claim).await
                }
            };
            results.push((hook.id.clone(), res));
        }
        results
    }

    /// Dispatch an event envelope to matching hooks.
    pub async fn dispatch_event(&self, envelope: &EventEnvelope) -> Vec<(String, DispatchResult)> {
        // For now, only handle claim-like events by inspecting the payload.
        // Future: map DomainEvent variants to HookTrigger directly.
        match &envelope.payload {
            DomainEvent::WorkPackageCreated(wp) => {
                info!("dispatching work-package created: {:?}", wp);
                Vec::new()
            }
            _ => {
                warn!(
                    "no claim-specific dispatch for event: {}",
                    envelope.payload.event_type()
                );
                Vec::new()
            }
        }
    }

    async fn dispatch_webhook(&self, url: &str, claim: &Claim) -> DispatchResult {
        let payload = serde_json::json!({
            "claim_id": claim.id,
            "resource": claim.resource,
            "agent_id": claim.agent_id,
            "state": claim.state,
        });
        match self.client.post(url).json(&payload).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    DispatchResult::Dispatched
                } else {
                    DispatchResult::Failed(format!("http {}", resp.status()))
                }
            }
            Err(e) => DispatchResult::Failed(e.to_string()),
        }
    }

    async fn dispatch_message(&self, topic: &str, claim: &Claim) -> DispatchResult {
        // Publish via agileplus-events message surface (stub for now).
        info!("message to topic={}: claim={}", topic, claim.id);
        DispatchResult::Dispatched
    }

    async fn dispatch_script(&self, command: &str, claim: &Claim) -> DispatchResult {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return DispatchResult::Failed("empty command".into());
        }
        let mut cmd = Command::new(parts[0]);
        cmd.args(&parts[1..]);
        cmd.env("CLAIM_ID", &claim.id);
        cmd.env("CLAIM_RESOURCE", &claim.resource);
        cmd.env("CLAIM_AGENT", &claim.agent_id);
        match cmd.output() {
            Ok(out) => {
                if out.status.success() {
                    DispatchResult::Dispatched
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    DispatchResult::Failed(format!("script exited: {stderr}"))
                }
            }
            Err(e) => DispatchResult::Failed(e.to_string()),
        }
    }
}
