// SPDX-License-Identifier: MIT OR Apache-2.0
//! Metric type — telemetry attached to features.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A recorded metric for a feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub id: i64,
    pub feature_id: Option<i64>,
    pub command: String,
    pub duration_ms: i64,
    pub agent_runs: i32,
    pub review_cycles: i32,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metric() -> Metric {
        Metric {
            id: 1,
            feature_id: Some(42),
            command: "cargo test".to_string(),
            duration_ms: 1500,
            agent_runs: 3,
            review_cycles: 1,
            metadata: Some(serde_json::json!({"key": "value"})),
            timestamp: DateTime::from_timestamp(0, 0).unwrap(),
        }
    }

    #[test]
    fn metric_fields_accessible() {
        let m = make_metric();
        assert_eq!(m.id, 1);
        assert_eq!(m.feature_id, Some(42));
        assert_eq!(m.command, "cargo test");
        assert_eq!(m.duration_ms, 1500);
        assert_eq!(m.agent_runs, 3);
        assert_eq!(m.review_cycles, 1);
    }

    #[test]
    fn metric_serializes_and_deserializes() {
        let m = make_metric();
        let json = serde_json::to_string(&m).unwrap();
        let back: Metric = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, m.id);
        assert_eq!(back.command, m.command);
        assert_eq!(back.duration_ms, m.duration_ms);
    }

    #[test]
    fn metric_optional_feature_id_can_be_none() {
        let mut m = make_metric();
        m.feature_id = None;
        let json = serde_json::to_string(&m).unwrap();
        let back: Metric = serde_json::from_str(&json).unwrap();
        assert!(back.feature_id.is_none());
    }
}
