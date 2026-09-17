// SPDX-License-Identifier: MIT OR Apache-2.0
//! Rule-based triage engine for synced GitHub items.
//!
//! Hexagonal design: `TriageRules` is a pure data config; `classify` is a
//! pure function — no I/O, no side effects.
//!
//! Traceability: FR-AGP-017

use agileplus_domain::domain::backlog::{BacklogPriority, Intent};
use serde::{Deserialize, Serialize};

// ── Port types (inbound) ────────────────────────────────────────────────────

/// A synced item from an external source (e.g. GitHub issue).
/// This is the primary input to the triage engine.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncedItem {
    pub title: String,
    pub body: Option<String>,
    /// Labels attached to the item on the external source.
    pub labels: Vec<String>,
}

// ── Domain output ───────────────────────────────────────────────────────────

/// The triage decision for a synced item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriageOutcome {
    pub priority: BacklogPriority,
    pub intent: Intent,
    /// Human-readable explanation of which rule matched.
    pub matched_rule: String,
}

// ── Config (data-driven rules) ──────────────────────────────────────────────

/// A single keyword rule.
/// The engine checks whether any of `keywords` appears (case-insensitive) in
/// the item's title, body, or labels.  First matching rule wins (ordered list).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageRule {
    /// Keywords to match against title/body/labels (case-insensitive).
    pub keywords: Vec<String>,
    /// Intent assigned when this rule fires.
    pub intent: Intent,
    /// Priority assigned when this rule fires.  If `None`, defaults to
    /// `intent.default_priority()`.
    pub priority: Option<BacklogPriority>,
    /// Short name for observability / matched_rule field.
    pub name: String,
}

/// The complete rule set used by the triage engine.
///
/// Rules are evaluated in order; the first match wins.  If no rule matches
/// the `default` outcome is used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageRules {
    /// Ordered list of rules; first match wins.
    pub rules: Vec<TriageRule>,
    /// Fallback when no rule matches.
    pub default: TriageOutcome,
}

impl TriageRules {
    /// Sensible built-in defaults matching the FR acceptance criteria.
    ///
    /// * bug / crash → `High` + `Bug`
    /// * docs        → `Low`  + `Docs`
    /// * unmatched   → `Medium` + `Task`
    pub fn default_rules() -> Self {
        Self {
            rules: vec![
                TriageRule {
                    name: "bug-keywords".to_owned(),
                    keywords: vec![
                        "bug".to_owned(),
                        "crash".to_owned(),
                        "error".to_owned(),
                        "panic".to_owned(),
                        "broken".to_owned(),
                        "regression".to_owned(),
                        "failing".to_owned(),
                        "exception".to_owned(),
                        "segfault".to_owned(),
                        "fix".to_owned(),
                    ],
                    intent: Intent::Bug,
                    priority: Some(BacklogPriority::High),
                },
                TriageRule {
                    name: "docs-keywords".to_owned(),
                    keywords: vec![
                        "docs".to_owned(),
                        "documentation".to_owned(),
                        "readme".to_owned(),
                        "changelog".to_owned(),
                        "typo".to_owned(),
                        "spelling".to_owned(),
                        "document".to_owned(),
                        "guide".to_owned(),
                        "tutorial".to_owned(),
                        "wiki".to_owned(),
                    ],
                    intent: Intent::Docs,
                    priority: Some(BacklogPriority::Low),
                },
                TriageRule {
                    name: "feature-keywords".to_owned(),
                    keywords: vec![
                        "feature".to_owned(),
                        "enhancement".to_owned(),
                        "implement".to_owned(),
                        "add".to_owned(),
                        "new".to_owned(),
                        "support".to_owned(),
                        "request".to_owned(),
                    ],
                    intent: Intent::Feature,
                    priority: None, // defaults to Medium
                },
            ],
            default: TriageOutcome {
                priority: BacklogPriority::Medium,
                intent: Intent::Task,
                matched_rule: "default".to_owned(),
            },
        }
    }
}

// ── Pure classify function ──────────────────────────────────────────────────

/// Classify a synced item using the provided rule set.
///
/// This is a **pure function**: no I/O, no mutation.
/// Rules are evaluated in declaration order; first match wins.
///
/// # Examples
/// ```
/// use agileplus_triage::engine::{SyncedItem, TriageRules, classify};
/// use agileplus_domain::domain::backlog::{Intent, BacklogPriority};
///
/// let rules = TriageRules::default_rules();
/// let item = SyncedItem {
///     title: "App crashes on login".to_owned(),
///     body: None,
///     labels: vec![],
/// };
/// let outcome = classify(&item, &rules);
/// assert_eq!(outcome.intent, Intent::Bug);
/// assert_eq!(outcome.priority, BacklogPriority::High);
/// ```
pub fn classify(item: &SyncedItem, rules: &TriageRules) -> TriageOutcome {
    // Build a single lower-cased search corpus from title + body + labels
    let corpus = build_corpus(item);

    for rule in &rules.rules {
        for kw in &rule.keywords {
            if corpus.contains(kw.as_str()) {
                let priority = rule
                    .priority
                    .unwrap_or_else(|| rule.intent.default_priority());
                return TriageOutcome {
                    priority,
                    intent: rule.intent,
                    matched_rule: rule.name.clone(),
                };
            }
        }
    }

    rules.default.clone()
}

/// Build a single lower-cased string combining title, body, and labels.
fn build_corpus(item: &SyncedItem) -> String {
    let mut parts = vec![item.title.to_lowercase()];
    if let Some(body) = &item.body {
        parts.push(body.to_lowercase());
    }
    for label in &item.labels {
        parts.push(label.to_lowercase());
    }
    parts.join(" ")
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use agileplus_domain::domain::backlog::{BacklogPriority, Intent};

    fn rules() -> TriageRules {
        TriageRules::default_rules()
    }

    // AC: bug keyword → High + Bug
    #[test]
    fn bug_keyword_in_title_gives_high_bug() {
        let item = SyncedItem {
            title: "App crashes on startup".to_owned(),
            body: None,
            labels: vec![],
        };
        let out = classify(&item, &rules());
        assert_eq!(out.intent, Intent::Bug);
        assert_eq!(out.priority, BacklogPriority::High);
        assert_eq!(out.matched_rule, "bug-keywords");
    }

    // AC: "bug" label → High + Bug
    #[test]
    fn bug_label_gives_high_bug() {
        let item = SyncedItem {
            title: "Something weird happening".to_owned(),
            body: None,
            labels: vec!["bug".to_owned()],
        };
        let out = classify(&item, &rules());
        assert_eq!(out.intent, Intent::Bug);
        assert_eq!(out.priority, BacklogPriority::High);
    }

    // AC: "crash" in body → High + Bug
    #[test]
    fn crash_keyword_in_body_gives_high_bug() {
        let item = SyncedItem {
            title: "Issue with login".to_owned(),
            body: Some("The app has a crash when submitting the form".to_owned()),
            labels: vec![],
        };
        let out = classify(&item, &rules());
        assert_eq!(out.intent, Intent::Bug);
        assert_eq!(out.priority, BacklogPriority::High);
    }

    // AC: docs → Low + Docs
    #[test]
    fn docs_keyword_gives_low_docs() {
        let item = SyncedItem {
            title: "Update the docs for the API".to_owned(),
            body: None,
            labels: vec![],
        };
        let out = classify(&item, &rules());
        assert_eq!(out.intent, Intent::Docs);
        assert_eq!(out.priority, BacklogPriority::Low);
        assert_eq!(out.matched_rule, "docs-keywords");
    }

    // AC: docs label
    #[test]
    fn docs_label_gives_low_docs() {
        let item = SyncedItem {
            title: "Improve onboarding".to_owned(),
            body: None,
            labels: vec!["documentation".to_owned()],
        };
        let out = classify(&item, &rules());
        assert_eq!(out.intent, Intent::Docs);
        assert_eq!(out.priority, BacklogPriority::Low);
    }

    // AC: unmatched → default Medium + Task
    #[test]
    fn unmatched_item_gives_medium_task() {
        let item = SyncedItem {
            title: "Quarterly review sync".to_owned(),
            body: Some("Let us meet and discuss".to_owned()),
            labels: vec![],
        };
        let out = classify(&item, &rules());
        assert_eq!(out.intent, Intent::Task);
        assert_eq!(out.priority, BacklogPriority::Medium);
        assert_eq!(out.matched_rule, "default");
    }

    // AC: rule precedence — bug rule comes before docs, so "bug in docs" → Bug
    #[test]
    fn rule_precedence_bug_before_docs() {
        let item = SyncedItem {
            title: "bug in the documentation page".to_owned(),
            body: None,
            labels: vec![],
        };
        let out = classify(&item, &rules());
        // "bug" appears first in corpus; bug-rule fires before docs-rule
        assert_eq!(out.intent, Intent::Bug);
        assert_eq!(out.priority, BacklogPriority::High);
    }

    // AC: empty input → default medium
    #[test]
    fn empty_item_gives_default() {
        let item = SyncedItem::default();
        let out = classify(&item, &rules());
        assert_eq!(out.intent, Intent::Task);
        assert_eq!(out.priority, BacklogPriority::Medium);
        assert_eq!(out.matched_rule, "default");
    }

    // AC: custom rule overrides built-in behaviour
    #[test]
    fn custom_rule_set_overrides_defaults() {
        let custom_rules = TriageRules {
            rules: vec![TriageRule {
                name: "security".to_owned(),
                keywords: vec!["auth".to_owned(), "token".to_owned()],
                intent: Intent::Bug,
                priority: Some(BacklogPriority::Critical),
            }],
            default: TriageOutcome {
                priority: BacklogPriority::Low,
                intent: Intent::Idea,
                matched_rule: "custom-default".to_owned(),
            },
        };

        let item = SyncedItem {
            title: "Auth token leaks in logs".to_owned(),
            body: None,
            labels: vec![],
        };
        let out = classify(&item, &custom_rules);
        assert_eq!(out.priority, BacklogPriority::Critical);
        assert_eq!(out.intent, Intent::Bug);

        let unmatched = SyncedItem {
            title: "Random thing".to_owned(),
            ..Default::default()
        };
        let default_out = classify(&unmatched, &custom_rules);
        assert_eq!(default_out.intent, Intent::Idea);
        assert_eq!(default_out.priority, BacklogPriority::Low);
    }

    // AC: feature label with no higher-priority rule match → Feature + Medium
    #[test]
    fn feature_keyword_gives_feature_medium() {
        let item = SyncedItem {
            title: "Add support for dark mode".to_owned(),
            body: None,
            labels: vec!["enhancement".to_owned()],
        };
        let out = classify(&item, &rules());
        assert_eq!(out.intent, Intent::Feature);
        assert_eq!(out.priority, BacklogPriority::Medium);
    }
}
