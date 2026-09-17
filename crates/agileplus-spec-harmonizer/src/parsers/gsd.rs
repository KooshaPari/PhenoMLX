//! GSD (Get Shit Done) parser.
//!
//! Format:
//! ```text
//! ## Task N: <title>
//! <free-form description>
//!
//! - [ ] acceptance item
//! - [x] done acceptance item
//!
//! ## Task N+1: <title>
//! ...
//! ```
//!
//! Captures the description as everything between the heading and the first
//! `## ` subheading, and accepts bullet-style acceptance criteria.

use crate::parsers::Parser;
use crate::{AcceptanceCriterion, WorkPackage};
use regex::Regex;

pub struct GsdParser;

impl Parser for GsdParser {
    fn parse(&self, text: &str) -> Result<Vec<WorkPackage>, String> {
        // Match `## Task N: title` or `## Task N - title` (lenient separators)
        let heading = Regex::new(r"(?m)^##\s+Task\s+(\d+)\s*[:\-]\s*(.+?)\s*$")
            .map_err(|e| format!("regex: {}", e))?;
        // Accept both lowercase `x` and uppercase `X` in checkbox markers.
        // Apply `(?i)` at the top so the `(x| )` alternation becomes
        // case-insensitive without restructuring the capture groups.
        let checkbox = Regex::new(r"(?i)^\s*-\s*\[(x| )\]\s+(.+?)\s*$")
            .map_err(|e| format!("regex: {}", e))?;
        let bullet = Regex::new(r"^\s*-\s+(.+?)\s*$")
            .map_err(|e| format!("regex: {}", e))?;

        let mut pkgs: Vec<WorkPackage> = Vec::new();
        let mut current: Option<WorkPackage> = None;
        let mut desc_buf = String::new();
        let mut acc_buf: Vec<AcceptanceCriterion> = Vec::new();
        let mut in_acc = false;

        let finalize = |pkg: &mut Option<WorkPackage>, desc: &mut String, acc: &mut Vec<AcceptanceCriterion>| {
            if let Some(mut p) = pkg.take() {
                let d = desc.trim().to_string();
                p.description = if d.is_empty() { "(no description)".into() } else { d };
                p.acceptance = std::mem::take(acc);
                // push and reset
                let slot = std::mem::replace(pkg, None);
                *pkg = Some(p);
            }
            desc.clear();
            acc.clear();
        };

        for line in text.lines() {
            if let Some(c) = heading.captures(line) {
                // flush previous
                if let Some(mut p) = current.take() {
                    p.description = {
                        let d = desc_buf.trim().to_string();
                        desc_buf.clear();
                        if d.is_empty() { "(no description)".into() } else { d }
                    };
                    p.acceptance = std::mem::take(&mut acc_buf);
                    pkgs.push(p);
                }
                let seq = c.get(1).unwrap().as_str();
                let title = c.get(2).unwrap().as_str().to_string();
                current = Some(WorkPackage {
                    id: format!("gsd-{}", seq),
                    title,
                    description: String::new(),
                    acceptance: Vec::new(),
                    source_format: "gsd".into(),
                    source_anchor: format!("task-{}", seq),
                });
                in_acc = false;
                continue;
            }
            if current.is_none() {
                continue;
            }
            // detect start of a new ## section (not a checkbox)
            if line.trim_start().starts_with("## ") && !line.trim_start().starts_with("## Task") {
                // another kind of heading — treat as description boundary
                in_acc = false;
                if let Some(c) = bullet.captures(line) {
                    desc_buf.push_str(&c[1]);
                    desc_buf.push('\n');
                }
                continue;
            }
            if let Some(c) = checkbox.captures(line) {
                acc_buf.push(AcceptanceCriterion {
                    text: c[2].to_string(),
                    done: c[1].eq_ignore_ascii_case("x"),
                });
                in_acc = true;
                continue;
            }
            if in_acc {
                if let Some(c) = bullet.captures(line) {
                    acc_buf.push(AcceptanceCriterion { text: c[1].to_string(), done: false });
                }
            } else {
                desc_buf.push_str(line);
                desc_buf.push('\n');
            }
        }
        if let Some(mut p) = current.take() {
            let d = desc_buf.trim().to_string();
            p.description = if d.is_empty() { "(no description)".into() } else { d };
            p.acceptance = acc_buf;
            pkgs.push(p);
        }
        let _ = finalize; // silence unused
        if pkgs.is_empty() {
            return Err("no GSD `## Task N:` headings found in input".into());
        }
        Ok(pkgs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::Parser;

    #[test]
    fn parses_two_gsd_tasks_with_acceptance() {
        let text = "## Task 1: First\ndesc 1\n\n- [ ] a\n- [x] b\n\n## Task 2: Second\ndesc 2\n";
        let out = GsdParser.parse(text).expect("parse");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "First");
        assert_eq!(out[0].acceptance.len(), 2);
        assert!(!out[0].acceptance[0].done);
        assert!(out[0].acceptance[1].done);
        assert_eq!(out[1].title, "Second");
    }

    #[test]
    fn errors_when_no_heading() {
        let out = GsdParser.parse("just plain text");
        assert!(out.is_err());
    }

    /// Regression test for codeant-ai finding #3 (Major):
    /// Uppercase `[X]` checkboxes must parse as done acceptance criteria.
    /// Previously the checkbox regex only matched lowercase `x`, so `- [X] b`
    /// fell through to the description branch and never created an
    /// `AcceptanceCriterion`.
    #[test]
    fn parses_uppercase_x_checkbox_as_done() {
        let text = "## Task 1: First\ndesc 1\n\n- [ ] a\n- [X] b\n- [x] c\n";
        let out = GsdParser.parse(text).expect("parse");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].acceptance.len(), 3, "all three checkboxes should parse");
        assert!(!out[0].acceptance[0].done, "[ ] should be not done");
        assert!(out[0].acceptance[1].done, "[X] uppercase should be done");
        assert!(out[0].acceptance[2].done, "[x] lowercase should be done");
        assert_eq!(out[0].acceptance[0].text, "a");
        assert_eq!(out[0].acceptance[1].text, "b");
        assert_eq!(out[0].acceptance[2].text, "c");
        assert!(
            !out[0].description.contains("[X]") && !out[0].description.contains("- ["),
            "checkbox lines must not leak into description"
        );
    }
}
