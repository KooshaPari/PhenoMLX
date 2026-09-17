//! BMAD-Method parser.
//!
//! Format:
//! ```text
//! ## Story <id>: <title>
//! As a <role>, I want <capability>, so that <outcome>.
//!
//! ## Acceptance Criteria
//! - criterion 1
//! - criterion 2
//! ```
//!
//! Both `## Acceptance Criteria` (crate-documented contract) and the legacy
//! `## Criteria` alias are recognized as the acceptance-block boundary.

use crate::parsers::Parser;
use crate::{AcceptanceCriterion, WorkPackage};
use regex::Regex;

pub struct BmadParser;

impl Parser for BmadParser {
    fn parse(&self, text: &str) -> Result<Vec<WorkPackage>, String> {
        let story = Regex::new(r"(?m)^##\s+Story\s+([A-Za-z0-9_\-]+)\s*[:\-]\s*(.+?)\s*$")
            .map_err(|e| format!("regex: {}", e))?;
        // Accept both `## Acceptance Criteria` (crate-level documented contract)
        // and the legacy `## Criteria` alias as the acceptance-block boundary.
        let crit = Regex::new(r"(?im)^##\s+(?:Acceptance\s+)?Criteria\s*$")
            .map_err(|e| format!("regex: {}", e))?;
        let bullet = Regex::new(r"^\s*-\s+(.+?)\s*$")
            .map_err(|e| format!("regex: {}", e))?;

        let mut pkgs: Vec<WorkPackage> = Vec::new();
        let mut current: Option<WorkPackage> = None;
        let mut desc = String::new();
        let mut accs: Vec<AcceptanceCriterion> = Vec::new();
        let mut in_acc = false;

        for line in text.lines() {
            if let Some(c) = story.captures(line) {
                if let Some(mut p) = current.take() {
                    let d = desc.trim().to_string();
                    p.description = if d.is_empty() { "(no description)".into() } else { d };
                    p.acceptance = std::mem::take(&mut accs);
                    pkgs.push(p);
                }
                let id = c.get(1).unwrap().as_str().to_string();
                let title = c.get(2).unwrap().as_str().to_string();
                current = Some(WorkPackage {
                    id: format!("bmad-{}", id),
                    title,
                    description: String::new(),
                    acceptance: Vec::new(),
                    source_format: "bmad".into(),
                    source_anchor: id,
                });
                in_acc = false;
                desc.clear();
                continue;
            }
            if current.is_none() { continue; }
            if crit.is_match(line) {
                in_acc = true;
                continue;
            }
            if line.trim_start().starts_with("## ") {
                in_acc = false;
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
        if let Some(mut p) = current.take() {
            let d = desc.trim().to_string();
            p.description = if d.is_empty() { "(no description)".into() } else { d };
            p.acceptance = accs;
            pkgs.push(p);
        }
        if pkgs.is_empty() {
            return Err("no BMAD `## Story <id>:` headings found".into());
        }
        Ok(pkgs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::Parser;

    #[test]
    fn parses_bmad_story_with_criteria() {
        let text = "## Story S1: Signup\nAs a user, I want to sign up.\n\n## Criteria\n- email verified\n- captcha passed\n";
        let out = BmadParser.parse(text).expect("parse");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "bmad-S1");
        assert_eq!(out[0].acceptance.len(), 2);
    }

    /// Regression test for codeant-ai finding #2 (Critical):
    /// `## Acceptance Criteria` (crate-documented contract) must be recognized
    /// as the acceptance-block boundary, not only the legacy `## Criteria`.
    #[test]
    fn parses_bmad_story_with_acceptance_criteria_heading() {
        let text = "## Story S1: Signup\nAs a user, I want to sign up.\n\n## Acceptance Criteria\n- email verified\n- captcha passed\n";
        let out = BmadParser.parse(text).expect("parse");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "bmad-S1");
        assert_eq!(
            out[0].acceptance.len(),
            2,
            "Acceptance Criteria bullets must populate acceptance, not description"
        );
        assert_eq!(out[0].acceptance[0].text, "email verified");
        assert_eq!(out[0].acceptance[1].text, "captcha passed");
        assert!(
            !out[0].description.contains("email verified"),
            "acceptance bullet must not leak into description"
        );
    }
}
