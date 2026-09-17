//! Rubric catalog: the machine-readable v38 audit rubric consumed by the
//! SpecKitty scoring engine.
//!
//! Parses and validates `PILLARS-CATALOG.json` (generated from the phenotype-org-audits
//! v38 catalog — see `data/gen_pillars_catalog.py`). This is the rubric-as-schema slice
//! of the SpecKitty enforcement engine (docs/design/SPECKITTY-SCORECARD-ENFORCEMENT.md §2).

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::{GovernanceError, Result};

/// A full rubric catalog: 12 clusters spanning pillar IDs L0–L122.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RubricCatalog {
    /// Catalog schema version (e.g. "1.0").
    pub version: String,
    /// Schema identifier (e.g. "phenotype/audit-v38").
    pub schema: String,
    /// Number of clusters declared.
    pub clusters: usize,
    /// Count of sub-pillars with full definitions enumerated.
    #[serde(default)]
    pub sub_pillars_enumerated: usize,
    /// Human note on provenance/coverage.
    #[serde(default)]
    pub note: String,
    /// The cluster/pillar entries.
    pub pillars: Vec<Pillar>,
}

/// One cluster (a pillar-ID range and its category).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pillar {
    /// Cluster id, e.g. "C03".
    pub cluster: String,
    /// Pillar-ID range, e.g. "L30" or "L81-L95".
    pub pillar_range: String,
    /// Category label.
    pub category: String,
    /// Source markdown path.
    pub source: String,
    /// Reference to prose defs when sub-pillars are not enumerated (L0–L80).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defs_ref: Option<String>,
    /// Scoring rule for this cluster.
    pub scoring: ScoringSpec,
    /// Enumerated sub-pillars (empty for `defs_ref` clusters).
    #[serde(default)]
    pub sub_pillars: Vec<SubPillar>,
}

/// Scoring scale + grade thresholds for a cluster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoringSpec {
    /// Scale label, e.g. "0-3".
    pub scale: String,
    /// Glyph per score level.
    pub glyphs: std::collections::BTreeMap<String, String>,
    /// Grade letter -> minimum pct.
    pub grade: std::collections::BTreeMap<String, u8>,
}

/// One sub-pillar with its acceptance criterion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubPillar {
    /// Sub-pillar id, e.g. "L30.1".
    pub id: String,
    /// Short title.
    pub title: String,
    /// One-line name/summary (may be null in source).
    #[serde(default)]
    pub name: Option<String>,
    /// Acceptance criterion prose.
    #[serde(default)]
    pub acceptance: Option<String>,
    /// Soft-optimizing goal.
    #[serde(default)]
    pub soft_goal: Option<String>,
    /// Evidence pattern hint, e.g. "file:line".
    #[serde(default)]
    pub evidence_pattern: String,
}

impl RubricCatalog {
    /// Parse a catalog from a JSON string.
    pub fn from_json(s: &str) -> Result<Self> {
        let catalog: RubricCatalog =
            serde_json::from_str(s).map_err(|e| GovernanceError::Rubric(format!("parse: {e}")))?;
        catalog.validate()?;
        Ok(catalog)
    }

    /// Load and validate a catalog from a file path.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let s = std::fs::read_to_string(path)
            .map_err(|e| GovernanceError::Rubric(format!("read {}: {e}", path.display())))?;
        Self::from_json(&s)
    }

    /// Validate structural invariants: cluster count matches, ids are unique,
    /// every cluster has a scoring spec, enumerated clusters have non-empty
    /// sub-pillars, and `sub_pillars_enumerated` matches the actual count.
    pub fn validate(&self) -> Result<()> {
        if self.pillars.len() != self.clusters {
            return Err(GovernanceError::Rubric(format!(
                "clusters={} but {} pillar entries present",
                self.clusters,
                self.pillars.len()
            )));
        }
        let mut seen = std::collections::HashSet::new();
        let mut counted = 0usize;
        for p in &self.pillars {
            if !seen.insert(p.cluster.as_str()) {
                return Err(GovernanceError::Rubric(format!(
                    "duplicate cluster id {}",
                    p.cluster
                )));
            }
            if p.scoring.glyphs.is_empty() || p.scoring.grade.is_empty() {
                return Err(GovernanceError::Rubric(format!(
                    "cluster {} missing scoring glyphs/grade",
                    p.cluster
                )));
            }
            // A cluster is either enumerated (has sub_pillars) or references prose defs.
            if p.sub_pillars.is_empty() && p.defs_ref.is_none() {
                return Err(GovernanceError::Rubric(format!(
                    "cluster {} has no sub_pillars and no defs_ref",
                    p.cluster
                )));
            }
            let mut sub_seen = std::collections::HashSet::new();
            for sp in &p.sub_pillars {
                if !sub_seen.insert(sp.id.as_str()) {
                    return Err(GovernanceError::Rubric(format!(
                        "duplicate sub-pillar id {} in cluster {}",
                        sp.id, p.cluster
                    )));
                }
            }
            counted += p.sub_pillars.len();
        }
        if self.sub_pillars_enumerated != 0 && self.sub_pillars_enumerated != counted {
            return Err(GovernanceError::Rubric(format!(
                "sub_pillars_enumerated={} but counted {}",
                self.sub_pillars_enumerated, counted
            )));
        }
        Ok(())
    }

    /// Total sub-pillars actually enumerated across all clusters.
    pub fn enumerated_count(&self) -> usize {
        self.pillars.iter().map(|p| p.sub_pillars.len()).sum()
    }

    /// Look up a cluster by id (e.g. "C03").
    pub fn cluster(&self, id: &str) -> Option<&Pillar> {
        self.pillars.iter().find(|p| p.cluster == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_json() -> &'static str {
        r#"{
          "version": "1.0",
          "schema": "phenotype/audit-v38",
          "clusters": 2,
          "sub_pillars_enumerated": 1,
          "note": "test",
          "pillars": [
            {
              "cluster": "C00", "pillar_range": "L0-L9", "category": "Architecture",
              "source": "audit-30-pillar/", "defs_ref": "audit-30-pillar-L{0..9}.md",
              "scoring": {"scale": "0-3", "glyphs": {"0": "✗"}, "grade": {"A": 90}},
              "sub_pillars": []
            },
            {
              "cluster": "C03", "pillar_range": "L30", "category": "Agent Readiness",
              "source": "audit-v38/catalog/L30-agent-readiness.md",
              "scoring": {"scale": "0-3", "glyphs": {"0": "✗", "3": "✓"}, "grade": {"A": 90, "F": 0}},
              "sub_pillars": [
                {"id": "L30.1", "title": "Spec & FR Clarity", "name": "FRs machine-readable",
                 "acceptance": "docs/functional_requirements.md exists", "soft_goal": "85% traceability",
                 "evidence_pattern": "file:line"}
              ]
            }
          ]
        }"#
    }

    #[test]
    fn parses_and_validates_minimal_catalog() {
        let c = RubricCatalog::from_json(minimal_json()).expect("valid catalog");
        assert_eq!(c.clusters, 2);
        assert_eq!(c.enumerated_count(), 1);
        assert_eq!(c.cluster("C03").unwrap().category, "Agent Readiness");
        assert!(c.cluster("C00").unwrap().defs_ref.is_some());
    }

    #[test]
    fn rejects_cluster_count_mismatch() {
        let bad = minimal_json().replace("\"clusters\": 2", "\"clusters\": 3");
        let err = RubricCatalog::from_json(&bad).unwrap_err();
        assert!(matches!(err, GovernanceError::Rubric(_)));
    }

    #[test]
    fn rejects_enumerated_count_mismatch() {
        let bad = minimal_json().replace("\"sub_pillars_enumerated\": 1", "\"sub_pillars_enumerated\": 9");
        let err = RubricCatalog::from_json(&bad).unwrap_err();
        assert!(matches!(err, GovernanceError::Rubric(_)));
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(RubricCatalog::from_json("{not json").is_err());
    }

    #[test]
    fn subpillar_acceptance_is_readable() {
        let c = RubricCatalog::from_json(minimal_json()).unwrap();
        let sp = &c.cluster("C03").unwrap().sub_pillars[0];
        assert_eq!(sp.id, "L30.1");
        assert!(sp.acceptance.as_ref().unwrap().contains("functional_requirements"));
    }
}
