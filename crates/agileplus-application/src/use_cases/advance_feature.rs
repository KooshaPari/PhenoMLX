// SPDX-License-Identifier: MIT OR Apache-2.0
//! Use case: Advance a Feature to the next lifecycle state.

use std::str::FromStr;
use std::sync::Arc;

use agileplus_domain::domain::state_machine::FeatureState;
use agileplus_domain::ports::events::{DomainEvent, DomainEventPublisher};
use agileplus_domain::ports::StoragePort;

use crate::dto::AdvanceFeatureCmd;
use crate::error::AppError;

/// Advances a Feature one step along the lifecycle state machine.
pub struct AdvanceFeature {
    repo: Arc<dyn StoragePort>,
    publisher: Arc<dyn DomainEventPublisher>,
}

impl AdvanceFeature {
    pub fn new(repo: Arc<dyn StoragePort>, publisher: Arc<dyn DomainEventPublisher>) -> Self {
        Self { repo, publisher }
    }

    pub async fn execute(&self, cmd: AdvanceFeatureCmd) -> Result<(), AppError> {
        let mut feature = self
            .repo
            .get_feature_by_id(cmd.feature_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("feature {}", cmd.feature_id)))?;

        let target = FeatureState::from_str(&cmd.target_state)
            .map_err(|e| AppError::Domain(agileplus_domain::error::DomainError::Validation(e)))?;

        let from = feature.state.to_string();
        // Validate via domain state machine (returns DomainError on bad transition)
        feature
            .transition(target)
            .map_err(|e| AppError::Domain(agileplus_domain::error::DomainError::Validation(e)))?;

        self.repo
            .update_feature_state(cmd.feature_id, feature.state)
            .await?;

        self.publisher
            .publish(DomainEvent::FeatureStateAdvanced {
                id: cmd.feature_id,
                from,
                to: feature.state.to_string(),
            })?;

        Ok(())
    }
}
