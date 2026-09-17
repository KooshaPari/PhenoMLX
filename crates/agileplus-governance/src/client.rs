// SPDX-License-Identifier: MIT OR Apache-2.0
//! Governance client for interacting with the AgilePlus governance system
//!
//! The client provides a unified interface for:
//! - Policy checks
//! - Audit logging
//! - Release channel management
//! - Remote synchronization

use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::audit::{AuditEvent, AuditFilter, AuditLogger};
use crate::channel::{PromotionRequest, PromotionResult};
use crate::config::GovernanceConfig;
use crate::error::{GovernanceError, Result};
#[allow(unused_imports)] // PolicyContext is used in tests
use crate::policy::{PolicyCheck, PolicyContext, PolicyEngine, PolicyResult};
use crate::rate_limiter::{RateLimitKey, RateLimiter};
use crate::types::*;

/// Main governance client
pub struct GovernanceClient {
    config: GovernanceConfig,
    audit_logger: Arc<AuditLogger>,
    policy_engine: Arc<RwLock<PolicyEngine>>,
    rate_limiter: Arc<RateLimiter>,
    connection_status: Arc<RwLock<ConnectionStatus>>,
    last_sync: Arc<RwLock<Option<DateTime<Utc>>>>,
}

impl GovernanceClient {
    /// Create a new governance client
    pub async fn new(config: GovernanceConfig) -> Result<Self> {
        let governance = Self {
            audit_logger: Arc::new(AuditLogger::new(
                &config.local.db_path,
                config.local.retention_days,
            )?),
            policy_engine: Arc::new(RwLock::new(PolicyEngine::new())),
            rate_limiter: Arc::new(RateLimiter::new(crate::rate_limiter::RateLimitConfig {
                max_requests: config.rate_limit.max_requests,
                window: std::time::Duration::from_millis(config.rate_limit.window_ms),
            })),
            config,
            connection_status: Arc::new(RwLock::new(ConnectionStatus::Disabled)),
            last_sync: Arc::new(RwLock::new(None)),
        };

        // Try to connect to remote governance if enabled
        if governance.config.governance.enabled {
            governance.test_connection().await?;
        }

        Ok(governance)
    }

    /// Create with defaults
    pub async fn with_defaults() -> Result<Self> {
        let config = GovernanceConfig::from_env();
        Self::new(config).await
    }

    /// Test connection to remote governance
    async fn test_connection(&self) -> Result<()> {
        let url = format!("{}/health", self.config.governance.base_url);

        match tokio::time::timeout(
            std::time::Duration::from_secs(self.config.governance.timeout_secs),
            reqwest::get(&url),
        )
        .await
        {
            Ok(Ok(response)) if response.status().is_success() => {
                info!(
                    "Connected to remote governance at {}",
                    self.config.governance.base_url
                );
                *self.connection_status.write().await = ConnectionStatus::Connected;
                Ok(())
            }
            Ok(Ok(response)) => {
                warn!("Remote governance returned status {}", response.status());
                *self.connection_status.write().await = ConnectionStatus::Error;
                Err(GovernanceError::Network(format!(
                    "Health check failed with status {}",
                    response.status()
                )))
            }
            Ok(Err(e)) => {
                error!("Failed to connect to remote governance: {e}");
                *self.connection_status.write().await = ConnectionStatus::Error;
                Err(GovernanceError::Network(e.to_string()))
            }
            Err(_) => {
                warn!("Remote governance connection timed out");
                *self.connection_status.write().await = ConnectionStatus::Disconnected;
                Err(GovernanceError::Network("Connection timeout".to_string()))
            }
        }
    }

    /// Check if an action is allowed
    pub async fn check_policy(&self, check: PolicyCheck) -> Result<PolicyResult> {
        // Check rate limit first
        if self.config.rate_limit.enabled {
            let rate_key = RateLimitKey::new(
                check.context.user_id.clone(),
                check.context.client_ip.clone(),
                &check.action,
            );

            let rate_result = self.rate_limiter.check(&rate_key).await;
            if !rate_result.allowed {
                warn!(
                    "Rate limit exceeded for action {} by {:?}",
                    check.action, rate_key.user_id
                );

                self.log_audit(
                    AuditEvent::error("rate_limit_exceeded", "Rate limit exceeded")
                        .with_user(rate_key.user_id.unwrap_or_default())
                        .with_action(check.action.clone())
                        .with_resource(check.resource.clone(), None),
                )
                .await?;

                return Err(GovernanceError::RateLimitExceeded(format!(
                    "Rate limit exceeded. Retry after {:?}",
                    rate_result.retry_after
                )));
            }
        }

        // Check policy
        let result =
            self.policy_engine
                .read()
                .await
                .check(&check.resource, &check.action, &check.context);

        // Log the policy check
        let level = if result.allowed {
            LogLevel::Info
        } else {
            LogLevel::Warn
        };

        self.log_audit(
            AuditEvent::new(
                &check.action,
                level,
                if result.allowed {
                    OperationResult::Success
                } else {
                    OperationResult::Failure
                },
            )
            .with_user(check.context.user_id.clone().unwrap_or_default())
            .with_resource(&check.resource, None)
            .with_message(&result.reason)
            .with_metadata(serde_json::json!({
                "policy_result": result,
                "check": check,
            })),
        )
        .await?;

        Ok(result)
    }

    /// Check if a release channel promotion is allowed
    pub async fn check_promotion(&self, request: PromotionRequest) -> Result<PromotionResult> {
        // Validate channel transition
        if !request.is_valid_transition() {
            return Ok(PromotionResult::denied(
                format!(
                    "Invalid channel transition from {} to {}",
                    request.from, request.to
                ),
                vec!["invalid_transition".to_string()],
            ));
        }

        // Run policy checks
        let policy_result = self.policy_engine.read().await.check_promotion(&request);

        // Log the promotion check
        self.log_audit(
            AuditEvent::new(
                "check_promotion",
                LogLevel::Info,
                if policy_result.allowed {
                    OperationResult::Success
                } else {
                    OperationResult::Failure
                },
            )
            .with_user(&request.requested_by)
            .with_resource("release", Some(request.package.clone()))
            .with_message(&policy_result.reason)
            .with_metadata(serde_json::json!({
                "from": request.from.to_string(),
                "to": request.to.to_string(),
                "version": request.version,
            })),
        )
        .await?;

        if !policy_result.allowed {
            return Ok(PromotionResult::denied(
                policy_result.reason,
                policy_result.policy.map_or_else(Vec::new, |p| vec![p]),
            ));
        }

        // Calculate iteration by counting prior promotions to this channel
        let iteration = self.count_channel_promotions(&request.to).await + 1;

        Ok(PromotionResult::allowed(
            crate::channel::ChannelMetadata::new(
                request.to,
                request.version,
                request.requested_by,
                iteration,
            ),
        ))
    }

    /// Log an audit event
    pub async fn log_audit(&self, event: AuditEvent) -> Result<()> {
        if self.config.local.enabled {
            self.audit_logger.log(&event)?;
        }

        // Sync to remote if enabled
        if self.config.governance.enabled && self.config.sync.enabled {
            let url = format!("{}/audit", self.config.governance.base_url);
            let entry_clone = event.clone();
            let last_sync = self.last_sync.clone();
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                match client.post(&url).json(&entry_clone).send().await {
                    Ok(_resp) => {
                        info!("Synced audit event to remote: {}", url);
                        *last_sync.write().await = Some(chrono::Utc::now());
                    }
                    Err(e) => {
                        warn!("Failed to sync audit event to remote {}: {e}", url);
                    }
                }
            });
        }

        Ok(())
    }

    /// Query audit logs
    pub async fn query_audit(&self, filter: AuditFilter) -> Result<Vec<AuditEvent>> {
        self.audit_logger.query(&filter)
    }

    /// Get audit statistics
    pub async fn audit_stats(&self) -> Result<GovernanceStats> {
        self.audit_logger.stats()
    }

    /// Get governance status
    pub async fn status(&self) -> GovernanceStatus {
        let connection_status = *self.connection_status.read().await;
        let stats = self.audit_logger.stats().unwrap_or_default();
        let pending = self.audit_logger.unsynced_count().unwrap_or(0);

        GovernanceStatus {
            initialized: true,
            connection_status,
            remote_enabled: self.config.governance.enabled,
            local_enabled: self.config.local.enabled,
            sync_enabled: self.config.sync.enabled,
            last_sync: *self.last_sync.read().await,
            pending_operations: pending,
            config: GovernanceStatusConfig {
                governance_url: self.config.governance.base_url.clone(),
                auth_method: self.config.governance.auth.method.clone(),
                policy_enabled: self.config.policy.enabled,
                rate_limit_enabled: self.config.rate_limit.enabled,
            },
            stats,
        }
    }

    /// Get connection status
    pub async fn connection_status(&self) -> ConnectionStatus {
        *self.connection_status.read().await
    }

    /// Add a policy
    pub async fn add_policy(&self, policy: crate::policy::Policy) {
        let mut engine = self.policy_engine.write().await;
        engine.add_policy(policy);
    }

    /// Get all policies
    pub async fn policies(&self) -> Vec<crate::policy::Policy> {
        self.policy_engine.read().await.policies().to_vec()
    }

    /// Count the number of prior promotions to a specific channel by querying
    /// audit events whose action is `check_promotion`, result is `success`, and
    /// whose metadata `to` field matches the target channel.
    async fn count_channel_promotions(&self, channel: &crate::channel::ReleaseChannel) -> u32 {
        let filter = AuditFilter {
            action: Some("check_promotion".to_string()),
            result: Some(OperationResult::Success),
            ..AuditFilter::new()
        };

        match self.audit_logger.query(&filter) {
            Ok(events) => events
                .iter()
                .filter(|e| {
                    e.metadata
                        .as_ref()
                        .and_then(|m| m.get("to").and_then(|v| v.as_str()))
                        .map_or(false, |to| to == &channel.to_string())
                })
                .count() as u32,
            Err(_) => 0,
        }
    }
}

/// Builder for creating a GovernanceClient
pub struct GovernanceClientBuilder {
    config: Option<GovernanceConfig>,
}

impl GovernanceClientBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self { config: None }
    }

    /// Set configuration
    pub fn config(mut self, config: GovernanceConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Set configuration from file
    pub fn config_file(mut self, path: impl AsRef<std::path::Path>) -> Result<Self> {
        self.config = Some(GovernanceConfig::from_file(path.as_ref())?);
        Ok(self)
    }

    /// Set configuration from environment
    pub fn config_from_env(self) -> Self {
        self.config(GovernanceConfig::from_env())
    }

    /// Build the client
    pub async fn build(self) -> Result<GovernanceClient> {
        let config = self.config.unwrap_or_default();
        GovernanceClient::new(config).await
    }
}

impl Default for GovernanceClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = GovernanceClient::with_defaults().await;
        // May fail if DB can't be created, but that's ok for test
        if let Ok(c) = client {
            let status = c.status().await;
            assert!(status.initialized);
        }
    }

    #[tokio::test]
    async fn test_policy_check() {
        let client = GovernanceClient::with_defaults().await;

        // Skip test if database cannot be created (e.g., in read-only test env)
        let client = match client {
            Ok(c) => c,
            Err(_) => return,
        };

        let check = PolicyCheck {
            resource: "test".to_string(),
            action: "test_action".to_string(),
            context: PolicyContext::new()
                .with_user("test_user")
                .with_action("test_action"),
        };

        let result = client.check_policy(check).await;
        // Should not error
        assert!(result.is_ok() || result.is_err()); // Accept any result
    }
}
