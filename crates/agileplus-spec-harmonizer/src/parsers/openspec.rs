//! OpenSpec parser.
//!
//! Format:
//! ```text
//! ## Spec <id> — <title>
//! <description>
//!
//! ## Acceptance
//! - criterion 1
//! - criterion 2
//! ```
//!
//! The acceptance block is delimited by a `## Acceptance` heading and ends
//! at the next `## ` heading or EOF.

use crate::parsers::Parser;
use crate::{AcceptanceCriterion, WorkPackage};
use regex::Regex;

pub struct OpenSpecParser;

impl Parser for OpenSpecParser {
    fn parse(&self, text: &str) -> Result<Vec<WorkPackage>, String> {
        let spec = Regex::new(r"(?m)^##\s+Spec\s+([A-Za-z0-9_\-]+)\s*[—\-:]\s*(.+?)\s*$")
            .map_err(|e| format!("regex: {}", e))?;
        // Accept both `## Acceptance Criteria` (crate-level documented contract)
        // and the legacy `## Acceptance` alias as the acceptance-block boundary.
        let acc = Regex::new(r"(?im)^##\s+(?:Acceptance\s+Criteria|Acceptance)\s*$")
            .map_err(|e| format!("regex: {}", e))?;
        let bullet = Regex::new(r"^\s*-\s+(.+?)\s*$")
            .map_err(|e| format!("regex: {}", e))?;

        let mut pkgs: Vec<WorkPackage> = Vec::new();
        let mut current: Option<WorkPackage> = None;
        let mut desc = String::new();
        let mut accs: Vec<AcceptanceCriterion> = Vec::new();
        let mut in_acc = false;
        // Section-skip state: once we encounter a non-`Spec`, non-`Acceptance`
        // `## ` heading (e.g. `## Notes`), subsequent non-heading lines must
        // NOT bleed into the current spec's description. They are dropped
        // until the next `## Spec` heading resets `skip_section = false`.
        let mut skip_section = false;

        let flush = |pkgs: &mut Vec<WorkPackage>, cur: &mut Option<WorkPackage>, desc: &mut String, accs: &mut Vec<AcceptanceCriterion>| {
            if let Some(mut p) = cur.take() {
                let d = desc.trim().to_string();
                p.description = if d.is_empty() { "(no description)".into() } else { d };
                p.acceptance = std::mem::take(accs);
                pkgs.push(p);
            }
            desc.clear();
        };

        for line in text.lines() {
            if let Some(c) = spec.captures(line) {
                flush(&mut pkgs, &mut current, &mut desc, &mut accs);
                let id = c.get(1).unwrap().as_str().to_string();
                let title = c.get(2).unwrap().as_str().to_string();
                current = Some(WorkPackage {
                    id: format!("openspec-{}", id),
                    title,
                    description: String::new(),
                    acceptance: Vec::new(),
                    source_format: "openspec".into(),
                    source_anchor: id,
                });
                in_acc = false;
                skip_section = false;
                continue;
            }
            if current.is_none() { continue; }
            if acc.is_match(line) {
                in_acc = true;
                continue;
            }
            if line.trim_start().starts_with("## ") {
                // Any non-Spec, non-Acceptance heading (e.g. `## Notes`) ends
                // the current spec's content; subsequent lines must not leak
                // into description. The flush of the current package happens
                // at the next `## Spec` heading (handled above).
                in_acc = false;
                skip_section = true;
                continue;
            }
            if skip_section {
                // Inside a non-target section: drop lines until next `## Spec`.
                continue;
            }
            if in_acc {
                if let Some(c) = bullet.captures(line) {
                    accs.push(AcceptanceCriterion { text: c[1].to_string(), done: false });
                }
            } else {
                desc.push_str(line);
                desc.push('\n');
            }
        }
        flush(&mut pkgs, &mut current, &mut desc, &mut accs);
        if pkgs.is_empty() {
            return Err("no OpenSpec `## Spec <id>` headings found".into());
        }
        Ok(pkgs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::Parser;

    #[test]
    fn parses_openspec_with_acceptance() {
        let text = "## Spec ABC-1 — Login Flow\nUsers can log in.\n\n## Acceptance\n- email + password work\n- MFA optional\n\n## Spec ABC-2 — Logout\nClick logout.\n";
        let out = OpenSpecParser.parse(text).expect("parse");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "openspec-ABC-1");
        assert_eq!(out[0].title, "Login Flow");
        assert_eq!(out[0].acceptance.len(), 2);
        assert_eq!(out[1].title, "Logout");
        assert_eq!(out[1].acceptance.len(), 0);
    }

    /// Regression test for codeant-ai finding #5 (Major):
    /// `## Acceptance Criteria` (crate-level documented contract) must be
    /// recognized as the acceptance-block boundary, not only the bare
    /// `## Acceptance`. Previously this caused silent loss of acceptance
    /// items that were then appended into description.
    #[test]
    fn parses_openspec_with_acceptance_criteria_heading() {
        let text = "## Spec ABC-1 — Login\nUsers can log in.\n\n## Acceptance Criteria\n- email + password work\n- MFA optional\n";
        let out = OpenSpecParser.parse(text).expect("parse");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "openspec-ABC-1");
        assert_eq!(
            out[0].acceptance.len(),
            2,
            "Acceptance Criteria bullets must populate acceptance"
        );
        assert_eq!(out[0].acceptance[0].text, "email + password work");
        assert_eq!(out[0].acceptance[1].text, "MFA optional");
        assert!(
            !out[0].description.contains("email + password work"),
            "acceptance bullet must not leak into description"
        );
    }

    /// Regression test for codeant-ai finding #6 (Major):
    /// Non-`Spec` headings (e.g. `## Notes`) must not leak content into the
    /// previous spec's description. Previously, lines under such headings
    /// were appended to `desc` because `in_acc` was reset to `false` and the
    /// `else` branch unconditionally appended.
    #[test]
    fn does_not_leak_non_spec_heading_into_description() {
        let text = "## Spec ABC-1 — Login\nUsers can log in.\n\n## Acceptance\n- email + password work\n\n## Notes\nExtra commentary\n\n## Spec ABC-2 — Logout\nClick logout.\n";
        let out = OpenSpecParser.parse(text).expect("parse");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "Login");
        assert_eq!(
            out[0].description, "Users can log in.",
            "Notes section must not leak into Login description"
        );
        assert!(
            !out[0].description.contains("Extra commentary"),
            "Notes content must not appear in any description"
        );
        assert_eq!(out[1].title, "Logout");
        assert_eq!(out[1].description, "Click logout.");
    }
}
