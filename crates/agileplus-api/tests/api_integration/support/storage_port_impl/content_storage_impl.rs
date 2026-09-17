use agileplus_domain::domain::backlog::{
    BacklogFilters, BacklogItem, BacklogPriority, BacklogStatus,
};
use agileplus_domain::domain::feature::Feature;
use agileplus_domain::domain::state_machine::FeatureState;
use agileplus_domain::domain::work_package::{WorkPackage, WpDependency, WpState};
use agileplus_domain::error::DomainError;
use agileplus_domain::ports::ContentStoragePort;
use async_trait::async_trait;

use super::super::storage::MockStorage;
use super::{backlog, feature, work_package};

#[async_trait]
impl ContentStoragePort for MockStorage {
    async fn create_feature(&self, f: &Feature) -> Result<i64, DomainError> {
        feature::create_feature(self, f).await
    }

    async fn get_feature_by_slug(
        &self,
        slug: &str,
    ) -> Result<Option<Feature>, DomainError> {
        feature::get_feature_by_slug(self, slug).await
    }

    async fn get_feature_by_id(
        &self,
        id: i64,
    ) -> Result<Option<Feature>, DomainError> {
        feature::get_feature_by_id(self, id).await
    }

    async fn update_feature_state(
        &self,
        id: i64,
        state: FeatureState,
    ) -> Result<(), DomainError> {
        feature::update_feature_state(self, id, state).await
    }

    async fn update_feature(
        &self,
        feature: &Feature,
    ) -> Result<(), DomainError> {
        feature::update_feature(self, feature).await
    }

    async fn list_features_by_state(
        &self,
        state: FeatureState,
    ) -> Result<Vec<Feature>, DomainError> {
        feature::list_features_by_state(self, state).await
    }

    async fn list_all_features(&self) -> Result<Vec<Feature>, DomainError> {
        feature::list_all_features(self).await
    }

    async fn get_backlog_item(
        &self,
        id: i64,
    ) -> Result<Option<BacklogItem>, DomainError> {
        backlog::get_backlog_item(self, id).await
    }

    async fn list_backlog_items(
        &self,
        filters: &BacklogFilters,
    ) -> Result<Vec<BacklogItem>, DomainError> {
        backlog::list_backlog_items(self, filters).await
    }

    async fn create_backlog_item(
        &self,
        item: &BacklogItem,
    ) -> Result<i64, DomainError> {
        backlog::create_backlog_item(self, item).await
    }

    async fn update_backlog_status(
        &self,
        id: i64,
        status: BacklogStatus,
    ) -> Result<(), DomainError> {
        backlog::update_backlog_status(self, id, status).await
    }

    async fn update_backlog_priority(
        &self,
        id: i64,
        priority: BacklogPriority,
    ) -> Result<(), DomainError> {
        backlog::update_backlog_priority(self, id, priority).await
    }

    async fn pop_next_backlog_item(
        &self,
    ) -> Result<Option<BacklogItem>, DomainError> {
        backlog::pop_next_backlog_item(self).await
    }

    async fn create_work_package(
        &self,
        wp: &WorkPackage,
    ) -> Result<i64, DomainError> {
        work_package::create_work_package(self, wp).await
    }

    async fn get_work_package(
        &self,
        id: i64,
    ) -> Result<Option<WorkPackage>, DomainError> {
        work_package::get_work_package(self, id).await
    }

    async fn update_wp_state(
        &self,
        id: i64,
        state: WpState,
    ) -> Result<(), DomainError> {
        work_package::update_wp_state(self, id, state).await
    }

    async fn update_work_package(
        &self,
        wp: &WorkPackage,
    ) -> Result<(), DomainError> {
        work_package::update_work_package(self, wp).await
    }

    async fn list_wps_by_feature(
        &self,
        feature_id: i64,
    ) -> Result<Vec<WorkPackage>, DomainError> {
        work_package::list_wps_by_feature(self, feature_id).await
    }

    async fn add_wp_dependency(
        &self,
        dep: &WpDependency,
    ) -> Result<(), DomainError> {
        work_package::add_wp_dependency(self, dep).await
    }

    async fn get_wp_dependencies(
        &self,
        wp_id: i64,
    ) -> Result<Vec<WpDependency>, DomainError> {
        work_package::get_wp_dependencies(self, wp_id).await
    }

    async fn get_ready_wps(
        &self,
        feature_id: i64,
    ) -> Result<Vec<WorkPackage>, DomainError> {
        work_package::get_ready_wps(self, feature_id).await
    }
}
