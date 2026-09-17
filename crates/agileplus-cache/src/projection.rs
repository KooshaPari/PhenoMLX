//! Projection cache for Feature and WorkPackage state.

use crate::store::CacheStore;
use agileplus_domain::domain::feature::Feature;
use agileplus_domain::domain::work_package::WorkPackage;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const PROJECTION_TTL_SECS: u64 = 60;

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("Cache error: {0}")]
    CacheError(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeatureProjection {
    pub feature: Feature,
    pub cached_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkPackageProjection {
    pub workpackage: WorkPackage,
    pub cached_at: chrono::DateTime<chrono::Utc>,
}

pub struct ProjectionCache<S: CacheStore> {
    store: S,
}

impl<S: CacheStore> ProjectionCache<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub async fn get_feature(
        &self,
        feature_id: i64,
    ) -> Result<Option<FeatureProjection>, ProjectionError> {
        self.store
            .get(&format!("feature:{feature_id}"))
            .await
            .map_err(|e| ProjectionError::CacheError(e.to_string()))
    }

    pub async fn set_feature(&self, feature: &Feature) -> Result<(), ProjectionError> {
        let projection = FeatureProjection {
            feature: feature.clone(),
            cached_at: chrono::Utc::now(),
        };
        self.store
            .set(
                &format!("feature:{}", feature.id),
                &projection,
                Some(Duration::from_secs(PROJECTION_TTL_SECS)),
            )
            .await
            .map_err(|e| ProjectionError::CacheError(e.to_string()))
    }

    pub async fn get_workpackage(
        &self,
        wp_id: i64,
    ) -> Result<Option<WorkPackageProjection>, ProjectionError> {
        self.store
            .get(&format!("wp:{wp_id}"))
            .await
            .map_err(|e| ProjectionError::CacheError(e.to_string()))
    }

    pub async fn set_workpackage(&self, wp: &WorkPackage) -> Result<(), ProjectionError> {
        let projection = WorkPackageProjection {
            workpackage: wp.clone(),
            cached_at: chrono::Utc::now(),
        };
        self.store
            .set(
                &format!("wp:{}", wp.id),
                &projection,
                Some(Duration::from_secs(PROJECTION_TTL_SECS)),
            )
            .await
            .map_err(|e| ProjectionError::CacheError(e.to_string()))
    }

    pub async fn invalidate_feature(&self, feature_id: i64) -> Result<(), ProjectionError> {
        self.store
            .delete(&format!("feature:{feature_id}"))
            .await
            .map_err(|e| ProjectionError::CacheError(e.to_string()))
    }

    pub async fn invalidate_workpackage(&self, wp_id: i64) -> Result<(), ProjectionError> {
        self.store
            .delete(&format!("wp:{wp_id}"))
            .await
            .map_err(|e| ProjectionError::CacheError(e.to_string()))
    }
}
