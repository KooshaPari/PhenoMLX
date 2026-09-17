// SPDX-License-Identifier: MIT OR Apache-2.0
//! Use case: Create a new Feature aggregate and persist it.

use std::sync::Arc;

use agileplus_domain::domain::feature::Feature;
use agileplus_domain::ports::events::{DomainEvent, DomainEventPublisher};
use agileplus_domain::ports::StoragePort;

use crate::dto::{CreateFeatureCmd, FeatureCreatedOutput};
use crate::error::AppError;

/// Creates a Feature, persists it, then publishes `FeatureCreated`.
pub struct CreateFeature {
    repo: Arc<dyn StoragePort>,
    publisher: Arc<dyn DomainEventPublisher>,
}

impl CreateFeature {
    pub fn new(repo: Arc<dyn StoragePort>, publisher: Arc<dyn DomainEventPublisher>) -> Self {
        Self { repo, publisher }
    }

    pub async fn execute(&self, cmd: CreateFeatureCmd) -> Result<FeatureCreatedOutput, AppError> {
        let spec_hash = cmd.spec_hash.unwrap_or([0u8; 32]);
        let feature = Feature::new(
            &cmd.slug,
            &cmd.friendly_name,
            spec_hash,
            cmd.target_branch.as_deref(),
        );

        let id = self.repo.create_feature(&feature).await?;

        self.publisher
            .publish(DomainEvent::FeatureCreated {
                id,
                slug: feature.slug.clone(),
            })?;

        let mut persisted = feature;
        persisted.id = id;

        Ok(FeatureCreatedOutput {
            id,
            feature: persisted,
        })
    }
}
