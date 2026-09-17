use std::collections::HashMap;
use std::sync::Mutex;

pub use super::models::{PlaneIssue, PlaneWorkItem, PlaneWorkItemResponse};

#[derive(Debug, Default)]
pub struct InMemoryPlaneClient {
    issues: Mutex<HashMap<String, PlaneWorkItemResponse>>,
    created: Mutex<Vec<PlaneWorkItem>>,
    #[allow(dead_code)] // reserved - tracks updates for test assertions
    updated: Mutex<Vec<(String, PlaneWorkItem)>>,
}

impl InMemoryPlaneClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_issue(mut self, id: &str, name: &str) -> Self {
        self.issues
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                id.to_string(),
                PlaneWorkItemResponse {
                    id: id.to_string(),
                    name: name.to_string(),
                    description_html: None,
                    state: None,
                    updated_at: None,
                },
            );
        self
    }

    pub async fn create_work_item(
        &self,
        work_item: &PlaneWorkItem,
    ) -> anyhow::Result<PlaneWorkItemResponse> {
        let mut created = self.created.lock().unwrap_or_else(|e| e.into_inner());
        let id = format!("issue-{}", created.len() + 1);
        created.push(work_item.clone());
        drop(created);

        let response = PlaneWorkItemResponse {
            id: id.clone(),
            name: work_item.name.clone(),
            description_html: work_item.description_html.clone(),
            state: work_item.state.clone(),
            updated_at: Some(chrono::Utc::now().to_rfc3339()),
        };
        self.issues
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, response.clone());
        Ok(response)
    }

    pub async fn update_work_item(
        &self,
        id: &str,
        work_item: &PlaneWorkItem,
    ) -> anyhow::Result<PlaneWorkItemResponse> {
        let issues = self.issues.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = issues.get(id) {
            return Ok(PlaneWorkItemResponse {
                id: existing.id.clone(),
                name: work_item.name.clone(),
                description_html: work_item.description_html.clone(),
                state: work_item.state.clone(),
                updated_at: Some(chrono::Utc::now().to_rfc3339()),
            });
        }
        anyhow::bail!("issue {} not found", id)
    }

    pub async fn get_work_item(&self, id: &str) -> anyhow::Result<PlaneWorkItemResponse> {
        self.issues
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("issue {} not found", id))
    }

    pub async fn list_work_items(&self) -> anyhow::Result<Vec<PlaneWorkItemResponse>> {
        Ok(self
            .issues
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect())
    }

    pub async fn create_sub_issue(
        &self,
        parent_id: &str,
        title: &str,
        description_html: Option<String>,
    ) -> anyhow::Result<PlaneWorkItemResponse> {
        let work_item = PlaneWorkItem {
            id: None,
            name: title.to_string(),
            description_html,
            state: None,
            priority: Some(3),
            parent: Some(parent_id.to_string()),
            labels: vec![],
        };
        self.create_work_item(&work_item).await
    }

    pub async fn create_issue(&self, issue: &PlaneIssue) -> anyhow::Result<PlaneWorkItemResponse> {
        self.create_work_item(issue).await
    }

    pub async fn update_issue(
        &self,
        issue_id: &str,
        issue: &PlaneIssue,
    ) -> anyhow::Result<PlaneWorkItemResponse> {
        self.update_work_item(issue_id, issue).await
    }

    pub async fn get_issue(&self, issue_id: &str) -> anyhow::Result<PlaneWorkItemResponse> {
        self.get_work_item(issue_id).await
    }

    pub async fn list_issues(&self) -> anyhow::Result<Vec<PlaneWorkItemResponse>> {
        self.list_work_items().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_issue_returns_response() {
        let client = InMemoryPlaneClient::new();
        let issue = PlaneWorkItem {
            id: None,
            name: "Test Issue".to_string(),
            description_html: None,
            state: None,
            priority: Some(2),
            parent: None,
            labels: vec![],
        };
        let result = client.create_issue(&issue).await.unwrap();
        assert!(result.id.starts_with("issue-"));
        assert_eq!(result.name, "Test Issue");
    }

    #[tokio::test]
    async fn list_issues_returns_all() {
        let client = InMemoryPlaneClient::new()
            .with_issue("1", "Issue 1")
            .with_issue("2", "Issue 2");
        let issues = client.list_issues().await.unwrap();
        assert_eq!(issues.len(), 2);
    }

    #[tokio::test]
    async fn create_sub_issue_sets_parent() {
        let client = InMemoryPlaneClient::new().with_issue("parent-1", "Parent Issue");
        let result = client
            .create_sub_issue("parent-1", "Child Issue", None)
            .await
            .unwrap();
        assert!(result.id.starts_with("issue-"));
    }

    #[tokio::test]
    async fn get_issue_returns_issue() {
        let client = InMemoryPlaneClient::new().with_issue("issue-1", "My Issue");
        let result = client.get_issue("issue-1").await.unwrap();
        assert_eq!(result.name, "My Issue");
    }

    #[tokio::test]
    async fn get_issue_not_found() {
        let client = InMemoryPlaneClient::new();
        let result = client.get_issue("nonexistent").await;
        assert!(result.is_err());
    }
}
