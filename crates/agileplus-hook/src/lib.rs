// SPDX-License-Identifier: MIT OR Apache-2.0
//! agileplus-hook — MEOW hook primitive.
//!
//! Hooks bind claim events to actions: webhook, message, or script.
//! The [`HookDispatcher`] matches incoming claim events against a
//! [`HookRegistry`] and dispatches the matched actions.
//!
//! Traceability: audit recs #15 (MEOW hook primitive)

pub mod dispatch;
pub mod registry;

pub use registry::HookRegistry;

use serde::{Deserialize, Serialize};

/// When a hook should fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookTrigger {
    OnClaim,
    OnRelease,
    OnExpire,
    OnTransfer,
}

/// What to do when the hook fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum HookAction {
    /// POST a JSON payload to a URL.
    Webhook { url: String },
    /// Publish a message to a topic.
    Message { topic: String },
    /// Execute a local shell command.
    Script { command: String },
}

/// A hook binds a trigger to an action with an optional regex filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub id: String,
    pub trigger: HookTrigger,
    pub action: HookAction,
    /// Optional regex filter on `claim.resource`.
    pub condition: Option<String>,
}

impl Hook {
    /// Create a new hook.
    pub fn new(
        id: impl Into<String>,
        trigger: HookTrigger,
        action: HookAction,
        condition: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            trigger,
            action,
            condition,
        }
    }
}
