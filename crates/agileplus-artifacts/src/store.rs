use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("artifact not found: {bucket}/{key}")]
    ArtifactNotFound { bucket: String, key: String },
    #[error("S3 operation failed: {0}")]
    S3(String),
}

pub type Result<T> = std::result::Result<T, ArtifactError>;

#[async_trait]
pub trait ArtifactStore: Send + Sync {
    async fn ensure_buckets(&self, buckets: &[&str]) -> Result<()>;
    async fn upload(
        &self,
        bucket: &str,
        key: &str,
        data: Bytes,
        content_type: Option<&str>,
    ) -> Result<()>;
    async fn download(&self, bucket: &str, key: &str) -> Result<Bytes>;
    async fn archive_old_events(&self, older_than_days: u32) -> Result<u64>;
    async fn health_check(&self) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct InMemoryArtifactStore {
    buckets: RwLock<HashMap<String, HashMap<String, Bytes>>>,
}

impl InMemoryArtifactStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn buckets(&self) -> Vec<String> {
        self.buckets.read().await.keys().cloned().collect()
    }
}

#[async_trait]
impl ArtifactStore for InMemoryArtifactStore {
    async fn ensure_buckets(&self, buckets: &[&str]) -> Result<()> {
        let mut store = self.buckets.write().await;
        for bucket in buckets {
            store.entry((*bucket).to_string()).or_default();
        }
        Ok(())
    }

    async fn upload(
        &self,
        bucket: &str,
        key: &str,
        data: Bytes,
        _content_type: Option<&str>,
    ) -> Result<()> {
        let mut store = self.buckets.write().await;
        store
            .entry(bucket.to_string())
            .or_default()
            .insert(key.to_string(), data);
        Ok(())
    }

    async fn download(&self, bucket: &str, key: &str) -> Result<Bytes> {
        let store = self.buckets.read().await;
        store
            .get(bucket)
            .and_then(|bucket_data| bucket_data.get(key))
            .cloned()
            .ok_or_else(|| ArtifactError::ArtifactNotFound {
                bucket: bucket.to_string(),
                key: key.to_string(),
            })
    }

    async fn archive_old_events(&self, older_than_days: u32) -> Result<u64> {
        let mut store = self.buckets.write().await;
        let Some(events) = store.get_mut("events-archive") else {
            return Ok(0);
        };
        let before = chrono::Utc::now() - chrono::Duration::days(older_than_days as i64);
        let before_ts = before.timestamp();
        let keys_to_remove: Vec<_> = events
            .keys()
            .filter(|key| {
                key.parse::<i64>()
                    .is_ok_and(|timestamp| timestamp < before_ts)
            })
            .cloned()
            .collect();
        let count = keys_to_remove.len() as u64;
        for key in keys_to_remove {
            events.remove(&key);
        }
        Ok(count)
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct S3ArtifactStore {
    endpoint: String,
    access_key: String,
    secret_key: String,
    region: String,
    bucket_prefix: String,
}

impl S3ArtifactStore {
    pub fn new(
        endpoint: String,
        access_key: String,
        secret_key: String,
        region: String,
        bucket_prefix: String,
    ) -> Self {
        Self {
            endpoint,
            access_key,
            secret_key,
            region,
            bucket_prefix,
        }
    }
}

#[async_trait]
impl ArtifactStore for S3ArtifactStore {
    async fn ensure_buckets(&self, buckets: &[&str]) -> Result<()> {
        info!(
            endpoint = %self.endpoint,
            region = %self.region,
            bucket_prefix = %self.bucket_prefix,
            bucket_count = buckets.len(),
            access_key_configured = !self.access_key.is_empty(),
            secret_key_configured = !self.secret_key.is_empty(),
            "S3ArtifactStore.ensure_buckets called"
        );
        Ok(())
    }

    async fn upload(
        &self,
        bucket: &str,
        key: &str,
        _data: Bytes,
        _content_type: Option<&str>,
    ) -> Result<()> {
        Err(ArtifactError::S3(format!(
            "upload not implemented for {bucket}/{key}"
        )))
    }

    async fn download(&self, bucket: &str, key: &str) -> Result<Bytes> {
        Err(ArtifactError::S3(format!(
            "download not implemented for {bucket}/{key}"
        )))
    }

    async fn archive_old_events(&self, older_than_days: u32) -> Result<u64> {
        Err(ArtifactError::S3(format!(
            "archive not implemented for events older than {older_than_days} days"
        )))
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_upload_download_round_trip() {
        let store = InMemoryArtifactStore::new();
        store.ensure_buckets(&["artifacts"]).await.unwrap();

        let body = Bytes::from_static(b"artifact-body");
        store
            .upload(
                "artifacts",
                "evidence/report.md",
                body.clone(),
                Some("text/markdown"),
            )
            .await
            .unwrap();

        assert_eq!(
            store
                .download("artifacts", "evidence/report.md")
                .await
                .unwrap(),
            body
        );
    }

    #[tokio::test]
    async fn in_memory_download_missing_key_returns_error() {
        let store = InMemoryArtifactStore::new();

        let err = store.download("artifacts", "missing").await.unwrap_err();

        assert!(matches!(err, ArtifactError::ArtifactNotFound { .. }));
    }

    #[tokio::test]
    async fn in_memory_archive_old_timestamp_keys() {
        let store = InMemoryArtifactStore::new();
        store.ensure_buckets(&["events-archive"]).await.unwrap();
        store
            .upload(
                "events-archive",
                "1",
                Bytes::from_static(b"old"),
                Some("application/json"),
            )
            .await
            .unwrap();

        assert_eq!(store.archive_old_events(1).await.unwrap(), 1);
    }
}
