//! Spec Kitty parser.
//!
//! Format:
//! ```text
//! ## Spec <id> - <title>
//! <description>
//!
//! ## Acceptance
//! - criterion 1
//! - criterion 2
//! ```
//!
//! Subset of OpenSpec with a hyphen separator (no em-dash). Keeps the parser
//! separate so the harmonizer can detect which sub-format is in use and
//! preserve the anchor.

use crate::parsers::Parser;
use crate::{AcceptanceCriterion, WorkPackage};
use regex::Regex;

pub struct KittyParser;

impl Parser for KittyParser {
    fn parse(&self, text: &str) -> Result<Vec<WorkPackage>, String> {
        let spec = Regex::new(r"(?m)^##\s+Spec\s+([A-Za-z0-9_\-]+)\s+-\s+(.+?)\s*$")
            .map_err(|e| format!("regex: {}", e))?;
        let acc = Regex::new(r"(?m)^##\s+Acceptance\s*$")
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

        for line in text.lines() {
            if let Some(c) = spec.captures(line) {
                if let Some(mut p) = current.take() {
                    let d = desc.trim().to_string();
                    p.description = if d.is_empty() { "(no description)".into() } else { d };
                    p.acceptance = std::mem::take(&mut accs);
                    pkgs.push(p);
                }
                let id = c.get(1).unwrap().as_str().to_string();
                let title = c.get(2).unwrap().as_str().to_string();
                current = Some(WorkPackage {
                    id: format!("kitty-{}", id),
                    title,
                    description: String::new(),
                    acceptance: Vec::new(),
                    source_format: "kitty".into(),
                    source_anchor: id,
                });
                in_acc = false;
                skip_section = false;
                desc.clear();
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
        if let Some(mut p) = current.take() {
            let d = desc.trim().to_string();
            p.description = if d.is_empty() { "(no description)".into() } else { d };
            p.acceptance = accs;
            pkgs.push(p);
        }
        if pkgs.is_empty() {
            return Err("no Spec Kitty `## Spec <id> - <title>` headings found".into());
        }
        Ok(pkgs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::Parser;

    #[test]
    fn parses_kitty_spec_hyphen_separator() {
        let text = "## Spec K-1 - Login\nUser logs in.\n\n## Acceptance\n- email + password\n";
        let out = KittyParser.parse(text).expect("parse");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "kitty-K-1");
        assert_eq!(out[0].title, "Login");
        assert_eq!(out[0].acceptance.len(), 1);
    }

    /// Regression test for codeant-ai finding #4 (Major):
    /// Non-`Spec` headings (e.g. `## Notes`) must not leak content into the
    /// previous spec's description. Previously, lines under such headings
    /// were appended to `desc` because `in_acc` was reset to `false` and the
    /// `else` branch unconditionally appended.
    #[test]
    fn does_not_leak_non_spec_heading_into_description() {
        let text = "## Spec K-1 - Login\nLine 1\n\n## Acceptance\n- bullet\n\n## Notes\nImplementation details\n\n## Spec K-2 - Logout\nClick logout.\n";
        let out = KittyParser.parse(text).expect("parse");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "Login");
        assert_eq!(
            out[0].description, "Line 1",
            "Notes section must not leak into Login description"
        );
        assert!(
            !out[0].description.contains("Implementation details"),
            "Notes content must not appear in any description"
        );
        assert_eq!(out[1].title, "Logout");
        assert_eq!(out[1].description, "Click logout.");
        assert_eq!(out[0].acceptance.len(), 1);
    }
}
