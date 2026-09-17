// SPDX-License-Identifier: MIT OR Apache-2.0
//! Backlog queue types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The intent/category of a backlog item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Intent {
    Bug,
    Feature,
    Idea,
    Task,
    /// Documentation work item (low priority by default).
    Docs,
}

impl fmt::Display for Intent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Intent::Bug => "bug",
            Intent::Feature => "feature",
            Intent::Idea => "idea",
            Intent::Task => "task",
            Intent::Docs => "docs",
        };
        write!(f, "{s}")
    }
}

impl FromStr for Intent {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bug" => Ok(Intent::Bug),
            "feature" => Ok(Intent::Feature),
            "idea" => Ok(Intent::Idea),
            "task" => Ok(Intent::Task),
            "docs" => Ok(Intent::Docs),
            _ => Err(format!("unknown Intent: {s}")),
        }
    }
}

impl Intent {
    /// Returns the default `BacklogPriority` for this intent category.
    pub fn default_priority(self) -> BacklogPriority {
        match self {
            Intent::Bug => BacklogPriority::High,
            Intent::Feature => BacklogPriority::Medium,
            Intent::Task => BacklogPriority::Medium,
            Intent::Idea => BacklogPriority::Low,
            Intent::Docs => BacklogPriority::Low,
        }
    }
}

/// Priority of a backlog item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BacklogPriority {
    Critical,
    High,
    Medium,
    Low,
}

impl fmt::Display for BacklogPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BacklogPriority::Critical => "critical",
            BacklogPriority::High => "high",
            BacklogPriority::Medium => "medium",
            BacklogPriority::Low => "low",
        };
        write!(f, "{s}")
    }
}

impl FromStr for BacklogPriority {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "critical" => Ok(BacklogPriority::Critical),
            "high" => Ok(BacklogPriority::High),
            "medium" => Ok(BacklogPriority::Medium),
            "low" => Ok(BacklogPriority::Low),
            _ => Err(format!("unknown BacklogPriority: {s}")),
        }
    }
}

/// Workflow status of a backlog item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BacklogStatus {
    New,
    Triaged,
    InProgress,
    Done,
    Dismissed,
}

impl fmt::Display for BacklogStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BacklogStatus::New => "new",
            BacklogStatus::Triaged => "triaged",
            BacklogStatus::InProgress => "in_progress",
            BacklogStatus::Done => "done",
            BacklogStatus::Dismissed => "dismissed",
        };
        write!(f, "{s}")
    }
}

impl FromStr for BacklogStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "new" => Ok(BacklogStatus::New),
            "triaged" => Ok(BacklogStatus::Triaged),
            "in_progress" => Ok(BacklogStatus::InProgress),
            "done" => Ok(BacklogStatus::Done),
            "dismissed" => Ok(BacklogStatus::Dismissed),
            _ => Err(format!("unknown BacklogStatus: {s}")),
        }
    }
}

/// Sort order for backlog queries.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BacklogSort {
    #[default]
    Age,
    Priority,
    Impact,
}

impl FromStr for BacklogSort {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "age" => Ok(BacklogSort::Age),
            "priority" => Ok(BacklogSort::Priority),
            "impact" => Ok(BacklogSort::Impact),
            _ => Err(format!("unknown BacklogSort: {s}")),
        }
    }
}

/// A single backlog item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklogItem {
    pub id: Option<i64>,
    pub title: String,
    pub description: String,
    pub intent: Intent,
    pub priority: BacklogPriority,
    pub status: BacklogStatus,
    pub source: String,
    pub feature_slug: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BacklogItem {
    /// Construct a new `BacklogItem` from a triage classification.
    ///
    /// Sets `priority` to the intent's default and `status` to `New`.
    pub fn from_triage(title: String, description: String, intent: Intent, source: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: None,
            title,
            description,
            priority: intent.default_priority(),
            intent,
            status: BacklogStatus::New,
            source,
            feature_slug: None,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Attach tags (builder-style).
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Attach an optional feature slug (builder-style).
    pub fn with_feature_slug(mut self, feature_slug: Option<String>) -> Self {
        self.feature_slug = feature_slug;
        self
    }
}

/// Filter parameters for backlog list queries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BacklogFilters {
    pub intent: Option<Intent>,
    pub status: Option<BacklogStatus>,
    pub priority: Option<BacklogPriority>,
    pub feature_slug: Option<String>,
    pub source: Option<String>,
    pub sort: BacklogSort,
    pub limit: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bug_intent_defaults_to_high_priority() {
        assert_eq!(Intent::Bug.default_priority(), BacklogPriority::High);
    }

    #[test]
    fn feature_intent_defaults_to_medium_priority() {
        assert_eq!(Intent::Feature.default_priority(), BacklogPriority::Medium);
    }

    #[test]
    fn idea_intent_defaults_to_low_priority() {
        assert_eq!(Intent::Idea.default_priority(), BacklogPriority::Low);
    }

    #[test]
    fn docs_intent_defaults_to_low_priority() {
        assert_eq!(Intent::Docs.default_priority(), BacklogPriority::Low);
    }

    #[test]
    fn task_intent_defaults_to_medium_priority() {
        assert_eq!(Intent::Task.default_priority(), BacklogPriority::Medium);
    }

    #[test]
    fn from_triage_sets_status_new_and_derives_priority() {
        let item = BacklogItem::from_triage(
            "Fix crash".into(),
            "App crashes on startup".into(),
            Intent::Bug,
            "github".into(),
        );
        assert_eq!(item.status, BacklogStatus::New);
        assert_eq!(item.priority, BacklogPriority::High);
        assert_eq!(item.title, "Fix crash");
        assert!(item.id.is_none());
        assert!(item.tags.is_empty());
    }

    #[test]
    fn intent_from_str_roundtrips() {
        for (s, expected) in [
            ("bug", Intent::Bug),
            ("feature", Intent::Feature),
            ("idea", Intent::Idea),
            ("task", Intent::Task),
            ("docs", Intent::Docs),
        ] {
            let intent: Intent = s.parse().unwrap();
            assert_eq!(intent, expected);
            assert_eq!(intent.to_string(), s);
        }
    }

    #[test]
    fn intent_from_str_rejects_unknown() {
        assert!("wip".parse::<Intent>().is_err());
    }

    #[test]
    fn backlog_priority_from_str_roundtrips() {
        for s in &["critical", "high", "medium", "low"] {
            let p: BacklogPriority = s.parse().unwrap();
            assert_eq!(p.to_string(), *s);
        }
    }

    #[test]
    fn backlog_status_from_str_roundtrips() {
        for s in &["new", "triaged", "in_progress", "done", "dismissed"] {
            let st: BacklogStatus = s.parse().unwrap();
            assert_eq!(st.to_string(), *s);
        }
    }
}
