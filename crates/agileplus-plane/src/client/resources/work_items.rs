use anyhow::Context;
use reqwest::Method;
use serde::Deserialize;

use super::{PlaneClient, PlaneIssue, PlaneWorkItem, PlaneWorkItemResponse, transport};

#[derive(Debug, Clone, Deserialize)]
struct PaginatedResponse<T> {
    results: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(dead_code)] // serde-deserialized - used for cursor-based pagination
    next: Option<String>,
}

impl PlaneClient {
    /// Create a work item in Plane.so.
    pub async fn create_work_item(
        &self,
        work_item: &PlaneWorkItem,
    ) -> anyhow::Result<PlaneWorkItemResponse> {
        self.acquire_token().await?;
        let resp = self
            .execute_request_json(Method::POST, &self.work_items_url(), work_item)
            .await
            .context("Plane.so create work item request failed")?;
        transport::read_json_response(resp, "parsing Plane.so response").await
    }

    /// Update an existing work item.
    pub async fn update_work_item(
        &self,
        work_item_id: &str,
        work_item: &PlaneWorkItem,
    ) -> anyhow::Result<PlaneWorkItemResponse> {
        self.acquire_token().await?;
        let resp = self
            .execute_request_json(Method::PATCH, &self.work_item_url(work_item_id), work_item)
            .await
            .context("Plane.so update work item request failed")?;
        transport::read_json_response(resp, "parsing Plane.so response").await
    }

    /// Get a work item by ID.
    pub async fn get_work_item(&self, work_item_id: &str) -> anyhow::Result<PlaneWorkItemResponse> {
        self.acquire_token().await?;
        let resp = self
            .execute_request_without_body(Method::GET, &self.work_item_url(work_item_id))
            .await
            .context("Plane.so get work item request failed")?;
        transport::read_json_response(resp, "parsing Plane.so response").await
    }

    /// List all work items in the project.
    pub async fn list_work_items(&self) -> anyhow::Result<Vec<PlaneWorkItemResponse>> {
        self.acquire_token().await?;
        let resp = self
            .execute_request_without_body(Method::GET, &self.work_items_url())
            .await
            .context("Plane.so list work items request failed")?;
        let text = transport::read_text_response(resp, "reading Plane.so list response").await?;
        if let Ok(items) = serde_json::from_str::<Vec<PlaneWorkItemResponse>>(&text) {
            return Ok(items);
        }
        let paginated: PaginatedResponse<PlaneWorkItemResponse> =
            serde_json::from_str(&text).context("parsing Plane.so paginated response")?;
        Ok(paginated.results)
    }

    /// Create a sub-issue (child work item) under a parent.
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

    // --- Back-compat aliases (outbound / sync use "issue" naming) ---

    /// Alias for [`Self::create_work_item`].
    pub async fn create_issue(&self, issue: &PlaneIssue) -> anyhow::Result<PlaneWorkItemResponse> {
        self.create_work_item(issue).await
    }

    /// Alias for [`Self::update_work_item`].
    pub async fn update_issue(
        &self,
        issue_id: &str,
        issue: &PlaneIssue,
    ) -> anyhow::Result<PlaneWorkItemResponse> {
        self.update_work_item(issue_id, issue).await
    }

    /// Alias for [`Self::get_work_item`].
    pub async fn get_issue(&self, issue_id: &str) -> anyhow::Result<PlaneWorkItemResponse> {
        self.get_work_item(issue_id).await
    }

    /// Alias for [`Self::list_work_items`].
    pub async fn list_issues(&self) -> anyhow::Result<Vec<PlaneWorkItemResponse>> {
        self.list_work_items().await
    }

    /// Alias for [`Self::add_work_item_to_module`].
    pub async fn add_issue_to_module(
        &self,
        plane_module_id: &str,
        plane_issue_id: &str,
    ) -> anyhow::Result<()> {
        self.add_work_item_to_module(plane_module_id, plane_issue_id)
            .await
    }

    /// Alias for [`Self::add_work_item_to_cycle`].
    pub async fn add_issue_to_cycle(
        &self,
        plane_cycle_id: &str,
        plane_issue_id: &str,
    ) -> anyhow::Result<()> {
        self.add_work_item_to_cycle(plane_cycle_id, plane_issue_id)
            .await
    }
}
