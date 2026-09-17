// SPDX-License-Identifier: MIT OR Apache-2.0
//! Aggregated scan report.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::dependency::Dependency;
use crate::vulnerability::{Severity, Vulnerability};

/// One finding: a dependency with its matched vulnerabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub dependency: Dependency,
    pub vulnerabilities: Vec<Vulnerability>,
}

/// Top-level scan summary.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub total: usize,
    pub with_findings: usize,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub unknown: usize,
}

/// The full report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub summary: Summary,
}

impl Report {
    /// Build a report from `findings`, computing the summary.
    pub fn from_findings(findings: Vec<Finding>) -> Self {
        let mut summary = Summary {
            total: findings.len(),
            ..Default::default()
        };
        for f in &findings {
            let mut has_match = false;
            for v in &f.vulnerabilities {
                if v.matches {
                    has_match = true;
                    match v.severity {
                        Severity::Critical => summary.critical += 1,
                        Severity::High => summary.high += 1,
                        Severity::Medium => summary.medium += 1,
                        Severity::Low => summary.low += 1,
                        Severity::Unknown => summary.unknown += 1,
                    }
                }
            }
            if has_match {
                summary.with_findings += 1;
            }
        }
        Self { findings, summary }
    }

    /// Returns true if at least one critical or high finding was found.
    pub fn has_blocking_findings(&self) -> bool {
        self.summary.critical > 0 || self.summary.high > 0
    }
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} deps scanned, {} with findings ({} critical, {} high, {} medium, {} low, {} unknown)",
            self.total,
            self.with_findings,
            self.critical,
            self.high,
            self.medium,
            self.low,
            self.unknown,
        )
    }
}
