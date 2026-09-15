//! Parser for the Hugging Face `model-index` YAML block.
//!
//! Real cards look like:
//! ```yaml
//! model-index:
//! - name: llm-eval-harness
//!   results:
//!   - task: {type: knowledge, name: mmlu}
//!     dataset:
//!       name: hendrycksTest
//!       type: text-only
//!     metrics:
//!     - name: acc
//!       value: 67.30
//!       verified: false
//!     source:
//!       url: https://huggingface.co/spaces/HuggingFaceH4/open_llm_leaderboard
//! ```
//!
//! [`parse_model_index`] flattens this nested shape into one
//! [`EvalRecord`] per `results:` row, taking the first parseable metric
//! as the canonical score:
//!
//! * `benchmark` = `"<dataset.name>/<task.name>"`
//! * `score`     = the metric's `value`
//! * `source`    = the entry's `source.url`
//!
//! Records that can't be paired (missing dataset / missing metric) are
//! silently dropped — extraction is best-effort.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::EvalError;

/// One benchmark result extracted from a `model-index` block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalRecord {
    /// `"<dataset_name>/<task_name>"`.
    pub benchmark: String,
    /// The metric value, parsed as `f64`.
    pub score: f64,
    /// Free-form provenance string (typically a URL).
    pub source: String,
    /// Parsed URL when `source` is one.
    pub source_url: Option<String>,
    /// ISO-ish date the eval was published, if upstream provides it.
    pub eval_date: Option<String>,
}

impl Default for EvalRecord {
    fn default() -> Self {
        Self {
            benchmark: String::new(),
            score: 0.0,
            source: String::new(),
            source_url: Option::None,
            eval_date: Option::None,
        }
    }
}

/// Permissive intermediate representation of a single metric.
#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)] // `name` / `verified` are kept for serde compatibility.
struct RawMetric {
    name: Option<String>,
    value: Option<serde_yaml::Value>,
    #[serde(default)]
    verified: bool,
}

/// One row of `results: […]` in the model-index.
#[derive(Debug, Default, Deserialize)]
struct RawResult {
    #[serde(default)]
    task: BTreeMap<String, serde_yaml::Value>,
    #[serde(default)]
    dataset: BTreeMap<String, serde_yaml::Value>,
    #[serde(default)]
    metrics: Vec<RawMetric>,
    #[serde(default)]
    source: BTreeMap<String, serde_yaml::Value>,
    #[serde(default)]
    date: Option<String>,
}

/// Top-level model-index wrapper.
#[derive(Debug, Default, Deserialize)]
struct RawModelIndex {
    #[serde(default, rename = "model-index")]
    model_index: Vec<RawModelIndexEntry>,
}

#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)] // `name` is deserialized for forward compatibility.
struct RawModelIndexEntry {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    results: Vec<RawResult>,
}

/// Parse a model-index YAML string and emit one [`EvalRecord`] per metric.
///
/// `yaml_text` is the full YAML body of the README front-matter — we look
/// for the top-level `model-index:` key (and fall back to a top-level
/// `results:` list). Empty / unparseable input yields an empty `Vec`.
pub fn parse_model_index(yaml_text: &str) -> Vec<EvalRecord> {
    if yaml_text.trim().is_empty() {
        return Vec::new();
    }

    let parsed: Result<RawModelIndex, _> = serde_yaml::from_str(yaml_text);
    let mut records: Vec<EvalRecord> = Vec::new();

    let idx = match parsed {
        Ok(i) => i,
        Err(_) => {
            // Fallback: try the bare `results:` form some cards use.
            return parse_bare_results(yaml_text);
        }
    };

    for entry in idx.model_index {
        for r in entry.results {
            push_records(&mut records, r);
        }
    }
    records
}

fn parse_bare_results(yaml_text: &str) -> Vec<EvalRecord> {
    #[derive(Debug, Default, Deserialize)]
    struct Bare {
        #[serde(default)]
        results: Vec<RawResult>,
    }
    let parsed: Result<Bare, _> = serde_yaml::from_str(yaml_text);
    let mut records = Vec::new();
    if let Ok(b) = parsed {
        for r in b.results {
            push_records(&mut records, r);
        }
    }
    records
}

fn push_records(records: &mut Vec<EvalRecord>, r: RawResult) {
    let dataset_name = r
        .dataset
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let task_name = r
        .task
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if dataset_name.is_empty() && task_name.is_empty() {
        return;
    }
    let benchmark = if task_name.is_empty() {
        dataset_name.clone()
    } else if dataset_name.is_empty() {
        task_name.clone()
    } else {
        format!("{dataset_name}/{task_name}")
    };
    let source_url = r
        .source
        .get("url")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let source = source_url.clone().unwrap_or_default();
    let eval_date = r.date.clone();
    // Pick the first parseable metric as the canonical score. Cards that
    // list multiple metrics (e.g. acc + acc_norm) for the same task still
    // produce one EvalRecord — callers who want per-metric granularity
    // should call this once per metric via a small wrapper.
    for m in r.metrics {
        let Some(value) = m.value else { continue };
        let Ok(score) = yaml_value_to_f64(&value) else {
            continue;
        };
        records.push(EvalRecord {
            benchmark: benchmark.clone(),
            score,
            source: source.clone(),
            source_url: source_url.clone(),
            eval_date: eval_date.clone(),
        });
        break;
    }
}

fn yaml_value_to_f64(v: &serde_yaml::Value) -> Result<f64, EvalError> {
    match v {
        serde_yaml::Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| EvalError::Parse(format!("non-f64 number: {n}"))),
        serde_yaml::Value::String(s) => s
            .parse::<f64>()
            .map_err(|e| EvalError::Parse(format!("bad numeric string {s:?}: {e}"))),
        serde_yaml::Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        other => Err(EvalError::Parse(format!("unsupported metric value: {other:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_yaml_returns_empty() {
        assert!(parse_model_index("").is_empty());
        assert!(parse_model_index("   \n\n").is_empty());
    }

    #[test]
    fn single_mmlu_record() {
        let yaml = r#"
model-index:
- name: llm-eval
  results:
  - task: {type: knowledge, name: mmlu}
    dataset:
      name: hendrycksTest
      type: text-only
    metrics:
    - name: acc
      value: 67.30
    source:
      url: https://example.com/leaderboard
"#;
        let recs = parse_model_index(yaml);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].benchmark, "hendrycksTest/mmlu");
        assert!((recs[0].score - 67.30).abs() < 1e-6);
        assert_eq!(recs[0].source_url.as_deref(), Some("https://example.com/leaderboard"));
    }

    #[test]
    fn multiple_metrics_per_entry_uses_first() {
        let yaml = r#"
model-index:
- results:
  - task: {name: arc}
    dataset: {name: ARC-Challenge}
    metrics:
    - {name: acc, value: 50.0}
    - {name: acc_norm, value: 51.5}
"#;
        let recs = parse_model_index(yaml);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].benchmark, "ARC-Challenge/arc");
        assert!((recs[0].score - 50.0).abs() < 1e-6);
    }
}