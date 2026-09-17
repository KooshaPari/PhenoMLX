//! Emitter: produce a TRAC-aligned representation (NDJSON) and Markdown index.

use crate::WorkPackage;

/// Escape characters that would otherwise corrupt a Markdown table cell.
/// Backslash-escape `|` and replace embedded newlines with `<br>` so the
/// generated row always has exactly three data columns regardless of title
/// content. CR is stripped.
fn escape_md_cell(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '|' => out.push_str("\\|"),
            '\r' => {} // drop CR
            '\n' => out.push_str("<br>"),
            c => out.push(c),
        }
    }
    out
}

/// Emit NDJSON: one WorkPackage per line.
pub fn emit_ndjson(pkgs: &[WorkPackage]) -> String {
    let mut out = String::new();
    for p in pkgs {
        let json = serde_json::to_string(p).unwrap_or_default();
        out.push_str(&json);
        out.push('\n');
    }
    out
}

/// Emit a Markdown index, grouped by source_format, with anchor links.
pub fn emit_markdown(pkgs: &[WorkPackage]) -> String {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, Vec<&WorkPackage>> = BTreeMap::new();
    for p in pkgs {
        groups.entry(p.source_format.clone()).or_default().push(p);
    }
    let mut out = String::new();
    out.push_str("# Harmonized Work Packages\n\n");
    out.push_str(&format!("Total: **{}** packages across **{}** formats.\n\n",
        pkgs.len(), groups.len()));
    for (fmt, group) in &groups {
        out.push_str(&format!("## {}\n\n", fmt));
        out.push_str("| ID | Title | Acceptance |\n|---|---|---|\n");
        for p in group {
            let acc = p.acceptance.len();
            let safe_title = escape_md_cell(&p.title);
            out.push_str(&format!("| `{}` | {} | {} |\n", p.id, safe_title, acc));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorkPackage;

    fn pkg(anchor: &str, fmt: &str) -> WorkPackage {
        WorkPackage {
            id: format!("{}-{}", fmt, anchor),
            title: format!("Title {}", anchor),
            description: "d".into(),
            acceptance: vec![],
            source_format: fmt.into(),
            source_anchor: anchor.into(),
        }
    }

    #[test]
    fn ndjson_one_line_per_package() {
        let pkgs = vec![pkg("1", "gsd"), pkg("2", "bmad")];
        let out = emit_ndjson(&pkgs);
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn markdown_groups_by_format() {
        let pkgs = vec![pkg("1", "gsd"), pkg("2", "gsd"), pkg("1", "bmad")];
        let out = emit_markdown(&pkgs);
        assert!(out.contains("## gsd"));
        assert!(out.contains("## bmad"));
        assert!(out.contains("Total: **3**"));
    }

    /// Regression test for codeant-ai finding #8 (Major):
    /// Markdown table cells must escape `|` (would split the row into extra
    /// columns) and replace embedded newlines with `<br>` (would otherwise
    /// break the row across two lines). The escaped output must keep the
    /// row structure intact regardless of title content.
    #[test]
    fn markdown_escapes_pipe_and_newline_in_title() {
        let mut p = pkg("1", "gsd");
        p.title = "Login | happy-path\nsecond line".to_string();
        let out = emit_markdown(&[p]);
        // Pipe must be backslash-escaped so it does not split the row.
        assert!(
            out.contains("Login \\| happy-path"),
            "pipe in title must be backslash-escaped; output was:\n{}",
            out
        );
        // Newline must be replaced with <br> so the row stays on one line.
        assert!(
            out.contains("Login \\| happy-path<br>second line"),
            "newline in title must become <br>; output was:\n{}",
            out
        );
        // The data row must contain exactly four unescaped pipes (one
        // leading + three column separators). The escape `\|` inside the
        // title must NOT be counted as a column separator.
        let data_row = out
            .lines()
            .find(|l| l.starts_with("| `gsd-1` |"))
            .expect("data row must exist");
        let total_pipes = data_row.chars().filter(|c| *c == '|').count();
        let escaped_pipes = data_row.matches("\\|").count();
        let unescaped_pipes = total_pipes - escaped_pipes;
        assert_eq!(
            unescaped_pipes, 4,
            "data row must have exactly 4 unescaped pipes (3 columns + leading); row was: {}",
            data_row
        );
        // Sanity: the literal "second line" must not appear on its own line.
        assert!(
            !out.lines().any(|l| l.trim() == "second line"),
            "newline must not split the row across two lines"
        );
    }
}
