// SPDX-License-Identifier: MIT OR Apache-2.0
//! Evidence types attached to a witness.

use serde::{Deserialize, Serialize};

/// Structured evidence that supports a witness verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum Evidence {
    /// Automated test results.
    TestResult {
        suite: String,
        passed: usize,
        failed: usize,
    },
    /// Human code review.
    CodeReview { reviewer: String, notes: String },
    /// Diff summary.
    Diff {
        from: String,
        to: String,
        lines_changed: usize,
    },
}
