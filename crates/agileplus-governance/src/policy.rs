// SPDX-License-Identifier: MIT OR Apache-2.0
//! Policy engine for governance decisions
//!
//! The policy engine evaluates rules against actions and determines
//! whether operations should be allowed or blocked.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::channel::{PromotionRequest, ReleaseChannel};

/// Policy check identifier
pub type PolicyCheckId = String;

/// A policy rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// Unique policy ID
    pub id: String,
    /// Policy name
    pub name: String,
    /// Policy description
    pub description: Option<String>,
    /// Resource this policy applies to
    pub resource: String,
    /// Action this policy applies to
    pub action: String,
    /// Whether to allow or deny
    pub effect: PolicyEffect,
    /// Policy conditions
    pub conditions: Vec<PolicyCondition>,
    /// Priority (higher = evaluated first)
    pub priority: i32,
    /// Whether policy is enabled
    pub enabled: bool,
}

impl Policy {
    /// Create a new policy
    pub fn new(
        resource: impl Into<String>,
        action: impl Into<String>,
        effect: PolicyEffect,
    ) -> Self {
        Self {
            id: format!("pol_{}", uuid::Uuid::new_v4()),
            name: String::new(),
            description: None,
            resource: resource.into(),
            action: action.into(),
            effect,
            conditions: Vec::new(),
            priority: 0,
            enabled: true,
        }
    }

    /// Set the name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a condition
    pub fn with_condition(mut self, condition: PolicyCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

/// Policy effect
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicyEffect {
    Allow,
    Deny,
}

/// A policy condition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PolicyCondition {
    /// Check against context value
    Equals {
        key: String,
        value: serde_json::Value,
    },
    /// Check if value contains
    Contains {
        key: String,
        value: serde_json::Value,
    },
    /// Check if value matches pattern
    Matches { key: String, pattern: String },
    /// Check if key exists
    Exists { key: String },
    /// Check if channel is at least
    MinChannel {
        key: String,
        channel: ReleaseChannel,
    },
    /// Check against environment
    Env { name: String, value: String },
    /// Logical NOT
    Not { condition: Box<PolicyCondition> },
    /// Logical AND
    And { conditions: Vec<PolicyCondition> },
    /// Logical OR
    Or { conditions: Vec<PolicyCondition> },
}

/// Result of a policy check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyResult {
    /// Whether the action is allowed
    pub allowed: bool,
    /// Reason for decision
    pub reason: String,
    /// Policy that matched (if any)
    pub policy: Option<String>,
    /// Evaluation details
    pub details: Vec<PolicyDetail>,
}

impl PolicyResult {
    /// Create an allowed result
    pub fn allowed(reason: impl Into<String>) -> Self {
        Self {
            allowed: true,
            reason: reason.into(),
            policy: None,
            details: Vec::new(),
        }
    }

    /// Create a denied result
    pub fn denied(reason: impl Into<String>, policy: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: reason.into(),
            policy: Some(policy.into()),
            details: Vec::new(),
        }
    }

    /// Add a detail
    pub fn add_detail(&mut self, detail: PolicyDetail) {
        self.details.push(detail);
    }
}

/// Details about policy evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDetail {
    pub policy: String,
    pub matched: bool,
    pub reason: String,
}

/// Context for policy evaluation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyContext {
    /// User making the request
    pub user_id: Option<String>,
    /// Client IP
    pub client_ip: Option<String>,
    /// Resource being accessed
    pub resource: Option<String>,
    /// Resource ID
    pub resource_id: Option<String>,
    /// Action being performed
    pub action: Option<String>,
    /// Current release channel
    pub channel: Option<ReleaseChannel>,
    /// Package name
    pub package: Option<String>,
    /// Version
    pub version: Option<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl PolicyContext {
    /// Create a new context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set user
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set resource
    pub fn with_resource(
        mut self,
        resource: impl Into<String>,
        resource_id: Option<String>,
    ) -> Self {
        self.resource = Some(resource.into());
        self.resource_id = resource_id;
        self
    }

    /// Set action
    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    /// Set channel
    pub fn with_channel(mut self, channel: ReleaseChannel) -> Self {
        self.channel = Some(channel);
        self
    }

    /// Get a value from context
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        match key {
            "user_id" => self.user_id.as_ref().map(|s| serde_json::json!(s)),
            "client_ip" => self.client_ip.as_ref().map(|s| serde_json::json!(s)),
            "resource" => self.resource.as_ref().map(|s| serde_json::json!(s)),
            "resource_id" => self.resource_id.as_ref().map(|s| serde_json::json!(s)),
            "action" => self.action.as_ref().map(|s| serde_json::json!(s)),
            "channel" => self
                .channel
                .as_ref()
                .map(|c| serde_json::json!(c.to_string())),
            "package" => self.package.as_ref().map(|s| serde_json::json!(s)),
            "version" => self.version.as_ref().map(|s| serde_json::json!(s)),
            _ => self.metadata.get(key).cloned(),
        }
    }
}

/// Policy check request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyCheck {
    /// Resource being accessed
    pub resource: String,
    /// Action being performed
    pub action: String,
    /// Context for evaluation
    pub context: PolicyContext,
}

/// Default policies for AgilePlus
pub fn default_policies() -> Vec<Policy> {
    vec![
        // Rate limiting policy
        Policy::new("governance", "rate_limit_exceeded", PolicyEffect::Deny)
            .with_name("Rate Limit Policy")
            .with_description("Deny requests that exceed rate limits")
            .with_priority(100),
        // Channel promotion policies
        Policy::new("release", "promote", PolicyEffect::Allow)
            .with_name("Allow Channel Promotion")
            .with_description("Allow valid channel promotions")
            .with_priority(50)
            .with_condition(PolicyCondition::Exists {
                key: "channel".to_string(),
            }),
        // Skip channel policy for low-risk packages
        Policy::new("release", "promote", PolicyEffect::Allow)
            .with_name("Allow Skip for Low Risk")
            .with_description("Allow skipping channels for low-risk packages")
            .with_priority(40)
            .with_condition(PolicyCondition::Equals {
                key: "risk_level".to_string(),
                value: serde_json::json!("low"),
            }),
        // Require tests for beta+
        Policy::new("release", "promote_beta", PolicyEffect::Deny)
            .with_name("Require Tests for Beta")
            .with_description("Beta promotions require passing tests")
            .with_priority(80)
            .with_condition(PolicyCondition::Equals {
                key: "tests_passed".to_string(),
                value: serde_json::json!(false),
            }),
        // Require security audit for RC
        Policy::new("release", "promote_rc", PolicyEffect::Deny)
            .with_name("Require Security Audit for RC")
            .with_description("RC promotions require security audit")
            .with_priority(90)
            .with_condition(PolicyCondition::Equals {
                key: "security_audit".to_string(),
                value: serde_json::json!(false),
            }),
        // Require rollback plan for production
        Policy::new("release", "promote_prod", PolicyEffect::Deny)
            .with_name("Require Rollback Plan for Production")
            .with_description("Production promotions require rollback plan")
            .with_priority(95)
            .with_condition(PolicyCondition::Equals {
                key: "rollback_plan".to_string(),
                value: serde_json::json!(false),
            }),
        // Admin bypass
        Policy::new("*", "bypass", PolicyEffect::Allow)
            .with_name("Admin Bypass")
            .with_description("Allow admins to bypass policies")
            .with_priority(1000)
            .with_condition(PolicyCondition::Equals {
                key: "is_admin".to_string(),
                value: serde_json::json!(true),
            }),
    ]
}

/// Policy engine
pub struct PolicyEngine {
    policies: Vec<Policy>,
    default_action: PolicyEffect,
}

impl PolicyEngine {
    /// Create a new policy engine with default policies
    pub fn new() -> Self {
        Self::with_policies(default_policies())
    }

    /// Create with custom policies
    pub fn with_policies(policies: Vec<Policy>) -> Self {
        let mut policies = policies;
        policies.sort_by_key(|b| std::cmp::Reverse(b.priority));

        Self {
            policies,
            default_action: PolicyEffect::Allow,
        }
    }

    /// Set default action
    pub fn with_default_action(mut self, action: PolicyEffect) -> Self {
        self.default_action = action;
        self
    }

    /// Add a policy
    pub fn add_policy(&mut self, policy: Policy) {
        self.policies.push(policy);
        self.policies.sort_by_key(|b| std::cmp::Reverse(b.priority));
    }

    /// Check if an action is allowed
    pub fn check(&self, resource: &str, action: &str, context: &PolicyContext) -> PolicyResult {
        for policy in &self.policies {
            if !policy.enabled {
                continue;
            }

            // Check if policy applies
            if policy.resource != "*" && policy.resource != resource {
                continue;
            }

            if policy.action != "*" && policy.action != action {
                continue;
            }

            // Check conditions
            let all_conditions_met = policy.conditions.is_empty()
                || policy
                    .conditions
                    .iter()
                    .all(|c| Self::evaluate_condition(c, context));

            if all_conditions_met {
                let reason = policy
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("Policy {} matched", policy.name));

                return match policy.effect {
                    PolicyEffect::Allow => PolicyResult::allowed(reason),
                    PolicyEffect::Deny => PolicyResult::denied(reason, &policy.name),
                };
            }
        }

        // No policy matched, use default
        match self.default_action {
            PolicyEffect::Allow => PolicyResult::allowed("No policy matched, default allow"),
            PolicyEffect::Deny => {
                PolicyResult::denied("No policy matched, default deny", "default")
            }
        }
    }

    /// Evaluate a single condition.
    ///
    /// Associated (no `&self`): condition evaluation depends only on the
    /// condition tree and the request context, not on engine state. Keeping it
    /// `&self`-free also satisfies `clippy::only_used_in_recursion`.
    fn evaluate_condition(condition: &PolicyCondition, context: &PolicyContext) -> bool {
        match condition {
            PolicyCondition::Equals { key, value } => {
                context.get(key).is_none_or(|v| v == *value)
            }
            PolicyCondition::Contains { key, value } => {
                context.get(key).is_some_and(|v| match (v, value) {
                    (serde_json::Value::String(s), serde_json::Value::String(pattern)) => {
                        s.contains(pattern)
                    }
                    _ => false,
                })
            }
            PolicyCondition::Matches { key, pattern } => context.get(key).is_some_and(|v| {
                if let serde_json::Value::String(s) = v {
                    regex::Regex::new(pattern)
                        .map(|r| r.is_match(s.as_str()))
                        .unwrap_or(false)
                } else {
                    false
                }
            }),
            PolicyCondition::Exists { key } => context.get(key).is_some(),
            PolicyCondition::MinChannel { key, channel } => context
                .get(key)
                .and_then(|v| {
                    if let serde_json::Value::String(s) = v {
                        s.parse::<ReleaseChannel>().ok()
                    } else {
                        None
                    }
                })
                .is_some_and(|c| c >= *channel),
            PolicyCondition::Env { name, value } => {
                std::env::var(name).ok().is_some_and(|v| v == *value)
            }
            PolicyCondition::Not { condition } => !Self::evaluate_condition(condition, context),
            PolicyCondition::And { conditions } => conditions
                .iter()
                .all(|c| Self::evaluate_condition(c, context)),
            PolicyCondition::Or { conditions } => conditions
                .iter()
                .any(|c| Self::evaluate_condition(c, context)),
        }
    }

    /// Check a promotion request
    pub fn check_promotion(&self, request: &PromotionRequest) -> PolicyResult {
        let mut context = PolicyContext::new();
        context.package = Some(request.package.clone());
        context.channel = Some(request.from);
        context.user_id = Some(request.requested_by.clone());
        context.version = Some(request.version.clone());

        // Add promotion-specific metadata
        context.metadata.insert(
            "skip_channels".to_string(),
            serde_json::json!(request.skips_channels()),
        );
        context.metadata.insert(
            "target_channel".to_string(),
            serde_json::json!(request.to.to_string()),
        );

        self.check("release", "promote", &context)
    }

    /// Get all policies
    pub fn policies(&self) -> &[Policy] {
        &self.policies
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_policy() {
        let engine = PolicyEngine::new();

        let context = PolicyContext::new()
            .with_user("test_user")
            .with_action("test_action");

        let result = engine.check("governance", "rate_limit_exceeded", &context);
        assert!(!result.allowed);
    }

    #[test]
    fn test_channel_promotion() {
        let engine = PolicyEngine::new();

        let request = PromotionRequest::new(
            "test-crate".to_string(),
            ReleaseChannel::Alpha,
            ReleaseChannel::Beta,
            "test_user".to_string(),
            "0.1.0".to_string(),
        );

        let result = engine.check_promotion(&request);
        // Alpha to Beta should be allowed (no conditions met for denial)
        assert!(result.allowed);
    }

    #[test]
    fn test_custom_policy() {
        let mut engine = PolicyEngine::new();

        engine.add_policy(
            Policy::new("test", "action", PolicyEffect::Deny)
                .with_name("Block Test")
                .with_condition(PolicyCondition::Equals {
                    key: "user_id".to_string(),
                    value: serde_json::json!("blocked_user"),
                }),
        );

        let context = PolicyContext::new().with_user("blocked_user");
        let result = engine.check("test", "action", &context);
        assert!(!result.allowed);

        let context = PolicyContext::new().with_user("allowed_user");
        let result = engine.check("test", "action", &context);
        assert!(result.allowed);
    }
}
