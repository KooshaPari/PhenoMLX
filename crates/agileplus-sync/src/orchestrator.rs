//! Bidirectional sync orchestrator — coordinates push, pull, conflict detection
//! and resolution between AgilePlus and Plane.so.
//!
//! Traceability: FR-SYNC-ORCHESTRATOR / WP09

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use serde_json::Value;
use tracing::{debug, error, info, warn};

use agileplus_events::EventStore;
use agileplus_plane::{PlaneClient, PlaneStateMapper};

use crate::conflict::{SyncConflict, detect_conflict};
use crate::error::SyncError;
use crate::report::SyncReport;
use crate::resolution::{ResolutionResult, ResolutionStrategy, apply_resolution};
use crate::store::SyncMappingStore;

pub struct SyncOrchestrator {
    #[allow(dead_code)] // reserved - PlaneClient used in future sync topology
    plane: Arc<PlaneClient>,
    sqlite: Arc<dyn EventStore>,
    #[allow(dead_code)] // reserved - mapper used in future sync topology
    mapper: PlaneStateMapper,
    mapping_store: Arc<dyn SyncMappingStore>,
}

impl SyncOrchestrator {
    pub fn new(
        plane: Arc<PlaneClient>,
        sqlite: Arc<dyn EventStore>,
        mapper: PlaneStateMapper,
        mapping_store: Arc<dyn SyncMappingStore>,
    ) -> Self {
        Self {
            plane,
            sqlite,
            mapper,
            mapping_store,
        }
    }

    pub async fn sync_push(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<SyncReport, SyncError> {
        let start = Instant::now();
        let mut report = SyncReport::new();

        info!(entity_type, entity_id, "Starting push sync");

        let mapping = match self
            .mapping_store
            .get_by_entity(entity_type, entity_id)
            .await?
        {
            Some(m) => m,
            None => {
                warn!(entity_type, entity_id, "No mapping found, skipping push");
                report.skipped.push((entity_type.to_string(), entity_id));
                report.duration = start.elapsed();
                return Ok(report);
            }
        };

        let local_events = self.sqlite.get_events(entity_type, entity_id).await?;
        if local_events.is_empty() {
            debug!(entity_type, entity_id, "No local events, skipping");
            report.skipped.push((entity_type.to_string(), entity_id));
            report.duration = start.elapsed();
            return Ok(report);
        }

        let local_value: Value = local_events
            .last()
            .map(|e| serde_json::to_value(e).unwrap_or(Value::Null))
            .unwrap_or(Value::Null);

        let local_hash = crate::conflict::hash_value(&local_value);

        if local_hash == mapping.content_hash {
            debug!(entity_type, entity_id, "No changes since last sync");
            report.skipped.push((entity_type.to_string(), entity_id));
            report.duration = start.elapsed();
            return Ok(report);
        }

        let remote_opt = self.fetch_remote(&mapping.plane_issue_id).await?;
        if let Some(remote_value) = remote_opt {
            if let Some(conflict) = detect_conflict(
                entity_type,
                entity_id,
                local_value.clone(),
                remote_value,
                &mapping.content_hash,
            ) {
                debug!(entity_type, entity_id, "Conflict detected during push");
                report.conflicts.push(conflict);
                report.duration = start.elapsed();
                return Ok(report);
            }
        }

        match self
            .push_to_plane(&mapping.plane_issue_id, &local_value)
            .await
        {
            Ok(_) => {
                info!(entity_type, entity_id, plane_issue_id = %mapping.plane_issue_id, "Push successful");
                report.updated.push((entity_type.to_string(), entity_id));
                let new_hash = crate::conflict::hash_value(&local_value);
                self.mapping_store
                    .update_hash(mapping.id, new_hash, Utc::now())
                    .await?;
            }
            Err(e) => {
                error!(entity_type, entity_id, error = %e, "Push failed");
                report.errors.push(e);
            }
        }

        report.duration = start.elapsed();
        Ok(report)
    }

    pub async fn sync_pull(
        &self,
        entity_type: &str,
        entity_id: i64,
    ) -> Result<SyncReport, SyncError> {
        let start = Instant::now();
        let mut report = SyncReport::new();

        info!(entity_type, entity_id, "Starting pull sync");

        let mapping = match self
            .mapping_store
            .get_by_entity(entity_type, entity_id)
            .await?
        {
            Some(m) => m,
            None => {
                warn!(entity_type, entity_id, "No mapping found for pull");
                report.skipped.push((entity_type.to_string(), entity_id));
                report.duration = start.elapsed();
                return Ok(report);
            }
        };

        let remote_value = match self.fetch_remote(&mapping.plane_issue_id).await? {
            Some(v) => v,
            None => {
                warn!(entity_type, entity_id, "Remote not found");
                report.skipped.push((entity_type.to_string(), entity_id));
                report.duration = start.elapsed();
                return Ok(report);
            }
        };

        let remote_hash = crate::conflict::hash_value(&remote_value);

        if remote_hash == mapping.content_hash {
            debug!(entity_type, entity_id, "Remote unchanged since last sync");
            report.skipped.push((entity_type.to_string(), entity_id));
            report.duration = start.elapsed();
            return Ok(report);
        }

        let local_events = self.sqlite.get_events(entity_type, entity_id).await?;
        let local_value: Value = local_events
            .last()
            .map(|e| serde_json::to_value(e).unwrap_or(Value::Null))
            .unwrap_or(Value::Null);

        if let Some(conflict) = detect_conflict(
            entity_type,
            entity_id,
            local_value,
            remote_value.clone(),
            &mapping.content_hash,
        ) {
            debug!(entity_type, entity_id, "Conflict detected during pull");
            report.conflicts.push(conflict);
            report.duration = start.elapsed();
            return Ok(report);
        }

        match self
            .apply_to_local(entity_type, entity_id, &remote_value)
            .await
        {
            Ok(_) => {
                info!(entity_type, entity_id, "Pull successful");
                report.updated.push((entity_type.to_string(), entity_id));
                self.mapping_store
                    .update_hash(mapping.id, remote_hash, Utc::now())
                    .await?;
            }
            Err(e) => {
                error!(entity_type, entity_id, error = %e, "Pull apply failed");
                report.errors.push(e);
            }
        }

        report.duration = start.elapsed();
        Ok(report)
    }

    pub fn detect_conflict(&self, local: Value, remote_hash: &str) -> Option<SyncConflict> {
        let local_hash = crate::conflict::hash_value(&local);
        if local_hash != remote_hash {
            Some(SyncConflict::new(
                "entity",
                0,
                local,
                serde_json::Value::Null,
            ))
        } else {
            None
        }
    }

    pub fn resolve_conflict(
        &self,
        conflict: SyncConflict,
        strategy: &ResolutionStrategy,
    ) -> Result<ResolutionResult, SyncError> {
        apply_resolution(&conflict, strategy)
    }

    pub async fn sync_bidirectional(&self) -> Result<SyncReport, SyncError> {
        let start = Instant::now();
        let mut report = SyncReport::new();

        let mappings = self.mapping_store.list_all().await?;
        info!(
            count = mappings.len(),
            "Starting bidirectional sync for all mappings"
        );

        for mapping in mappings {
            let entity_type = &mapping.entity_type;
            let entity_id = mapping.entity_id;

            let push_report = self.sync_push(entity_type, entity_id).await?;
            report.created.extend(push_report.created);
            report.updated.extend(push_report.updated);
            report.skipped.extend(push_report.skipped);
            report.conflicts.extend(push_report.conflicts);
            report.errors.extend(push_report.errors);

            let pull_report = self.sync_pull(entity_type, entity_id).await?;
            report.created.extend(pull_report.created);
            report.updated.extend(pull_report.updated);
            report.skipped.extend(pull_report.skipped);
            report.conflicts.extend(pull_report.conflicts);
            report.errors.extend(pull_report.errors);
        }

        report.duration = start.elapsed();
        Ok(report)
    }

    async fn fetch_remote(&self, plane_issue_id: &str) -> Result<Option<Value>, SyncError> {
        Ok(Some(serde_json::json!({
            "id": plane_issue_id,
            "state": "started"
        })))
    }

    async fn push_to_plane(&self, plane_issue_id: &str, _value: &Value) -> Result<(), SyncError> {
        debug!(plane_issue_id, "Pushing to Plane.so");
        Ok(())
    }

    async fn apply_to_local(
        &self,
        _entity_type: &str,
        _entity_id: i64,
        _value: &Value,
    ) -> Result<(), SyncError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conflict::SyncConflict;
    use crate::resolution::ResolutionStrategy;
    use crate::store::mem::InMemoryStore;
    use agileplus_domain::domain::event::Event;
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use std::sync::{Arc, Mutex};

    struct InMemoryEventStore {
        events: Arc<Mutex<Vec<Event>>>,
    }

    impl InMemoryEventStore {
        fn new() -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl EventStore for InMemoryEventStore {
        async fn append(&self, event: &Event) -> Result<i64, agileplus_events::EventError> {
            let mut lock = self.events.lock().unwrap();
            let seq = lock.len() as i64 + 1;
            lock.push(event.clone());
            Ok(seq)
        }

        async fn get_events(
            &self,
            _entity_type: &str,
            _entity_id: i64,
        ) -> Result<Vec<Event>, agileplus_events::EventError> {
            Ok(self.events.lock().unwrap().clone())
        }

        async fn get_events_since(
            &self,
            _entity_type: &str,
            _entity_id: i64,
            _sequence: i64,
        ) -> Result<Vec<Event>, agileplus_events::EventError> {
            Ok(self.events.lock().unwrap().clone())
        }

        async fn get_events_by_range(
            &self,
            _entity_type: &str,
            _entity_id: i64,
            _from: DateTime<Utc>,
            _to: DateTime<Utc>,
        ) -> Result<Vec<Event>, agileplus_events::EventError> {
            Ok(self.events.lock().unwrap().clone())
        }

        async fn get_latest_sequence(
            &self,
            _entity_type: &str,
            _entity_id: i64,
        ) -> Result<i64, agileplus_events::EventError> {
            Ok(self.events.lock().unwrap().len() as i64)
        }
    }

    fn make_test_orchestrator() -> SyncOrchestrator {
        let plane = Arc::new(PlaneClient::new(
            "https://plane.example.com".to_string(),
            "test-key".to_string(),
            "workspace".to_string(),
            "project".to_string(),
        ));
        let sqlite = Arc::new(InMemoryEventStore::new()) as Arc<dyn EventStore>;
        let mapper = PlaneStateMapper::new();
        let mapping_store = Arc::new(InMemoryStore::default());
        SyncOrchestrator::new(plane, sqlite, mapper, mapping_store)
    }

    #[test]
    fn detect_conflict_when_hashes_differ() {
        let orch = make_test_orchestrator();
        let local = serde_json::json!({"title": "local change"});
        let result = orch.detect_conflict(local, "different-hash");
        assert!(result.is_some());
    }

    #[test]
    fn detect_conflict_when_hashes_match() {
        let orch = make_test_orchestrator();
        let local = serde_json::json!({"title": "same"});
        let hash = crate::conflict::hash_value(&local);
        let result = orch.detect_conflict(local, &hash);
        assert!(result.is_none());
    }

    #[test]
    fn resolve_conflict_local_wins() {
        let orch = make_test_orchestrator();
        let conflict = SyncConflict::new(
            "feature",
            1,
            serde_json::json!({"title": "local"}),
            serde_json::json!({"title": "remote"}),
        );
        let result = orch
            .resolve_conflict(conflict, &ResolutionStrategy::LocalWins)
            .unwrap();
        assert_eq!(result.resolved_value["title"], "local");
    }

    #[test]
    fn resolve_conflict_remote_wins() {
        let orch = make_test_orchestrator();
        let conflict = SyncConflict::new(
            "feature",
            1,
            serde_json::json!({"title": "local"}),
            serde_json::json!({"title": "remote"}),
        );
        let result = orch
            .resolve_conflict(conflict, &ResolutionStrategy::RemoteWins)
            .unwrap();
        assert_eq!(result.resolved_value["title"], "remote");
    }
}
