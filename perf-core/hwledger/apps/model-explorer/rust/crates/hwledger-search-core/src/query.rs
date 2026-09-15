//! Caller-facing query and result shapes.

use serde::{Deserialize, Serialize};

use crate::taxonomy::faceted::Facets;

/// A complete search request as constructed by the CLI, server, or MCP front
/// ends.
///
/// `sort` is an optional sort hint (e.g. `"downloads"`, `"last_modified"`,
/// `"agentic_fit"`); the index layer decides whether/how to honor it.
/// `limit` caps the total returned rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Query {
    /// Free-text query.
    #[serde(default)]
    pub text: String,
    /// Structured filters.
    #[serde(default)]
    pub facets: Facets,
    /// Optional sort key (interpretation up to the index backend).
    #[serde(default)]
    pub sort: Option<String>,
    /// Maximum number of results to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    25
}

impl Default for Query {
    fn default() -> Self {
        Self {
            text: String::new(),
            facets: Facets::default(),
            sort: None,
            limit: default_limit(),
        }
    }
}

impl Query {
    /// Build a query with a free-text string and a default everything else.
    pub fn text<S: Into<String>>(s: S) -> Self {
        Self {
            text: s.into(),
            ..Self::default()
        }
    }

    /// Replace `facets` and return the new query (builder-style).
    pub fn with_facets(mut self, f: Facets) -> Self {
        self.facets = f;
        self
    }

    /// Replace `sort` and return the new query.
    pub fn with_sort<S: Into<String>>(mut self, sort: S) -> Self {
        self.sort = Some(sort.into());
        self
    }

    /// Replace `limit` and return the new query.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// A single search result returned across all layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FusedResult {
    /// `source::id`.
    pub id: String,
    /// Final, post-fusion, post-reranking score.
    pub score: f32,
    /// Facets resolved against this result (post-retrieval; useful for
    /// downstream drill-down UIs).
    pub facets: Facets,
    /// Optional raw payload (e.g. the tantivy stored-doc JSON), useful for
    /// the CLI and server layers that want to render extra detail.
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

impl Default for FusedResult {
    fn default() -> Self {
        Self {
            id: String::new(),
            score: 0.0,
            facets: Facets::default(),
            payload: None,
        }
    }
}

impl FusedResult {
    /// Build a minimal result row.
    pub fn new<I: Into<String>>(id: I, score: f32) -> Self {
        Self {
            id: id.into(),
            score,
            facets: Facets::default(),
            payload: None,
        }
    }

    /// Attach a payload and return the result (builder-style).
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Attach resolved facets and return the result (builder-style).
    pub fn with_facets(mut self, facets: Facets) -> Self {
        self.facets = facets;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let q = Query::default();
        assert_eq!(q.text, "");
        assert!(q.facets.kinds.is_empty());
        assert_eq!(q.limit, 25);
        assert!(q.sort.is_none());
    }

    #[test]
    fn builders() {
        let q = Query::text("hello world")
            .with_sort("downloads")
            .with_limit(50);
        assert_eq!(q.text, "hello world");
        assert_eq!(q.sort.as_deref(), Some("downloads"));
        assert_eq!(q.limit, 50);
    }

    #[test]
    fn fused_result_round_trip() {
        let r = FusedResult::new("hf::org/name", 0.42)
            .with_payload(serde_json::json!({"sha": "abc123"}));
        let j = serde_json::to_string(&r).unwrap();
        let back: FusedResult = serde_json::from_str(&j).unwrap();
        assert_eq!(back, r);
    }
}
