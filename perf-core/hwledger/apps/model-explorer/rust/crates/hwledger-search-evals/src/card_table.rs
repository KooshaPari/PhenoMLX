//! Markdown-table extractor for benchmark rows.
//!
//! Many HF model cards list their headline numbers in a table like:
//!
//! ```markdown
//! | Benchmark | Score |
//! |-----------|-------|
//! | MMLU      | 67.30 |
//! | GSM8K     | 52.10 |
//! ```
//!
//! [`parse_card_table`] walks every line of the input, detects any block
//! of two-or-more `|`-delimited rows where the header contains one of
//! `"benchmark"`, `"metric"`, or `"eval"` AND a column whose header
//! looks numeric ("score", "value", "acc", "%", …), and emits a
//! [`CardRow`] per data row. Tables without a numeric column are
//! silently skipped.

use serde::{Deserialize, Serialize};

/// One row extracted from a markdown benchmark table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardRow {
    /// Benchmark / dataset name (left-most non-empty cell, or whatever
    /// cell sits in the column the header called "benchmark").
    pub benchmark: String,
    /// Numeric score from the matched numeric column.
    pub score: f64,
}

impl Default for CardRow {
    fn default() -> Self {
        Self {
            benchmark: String::new(),
            score: 0.0,
        }
    }
}

/// Parse markdown and extract one [`CardRow`] per row in every detected
/// benchmark table.
pub fn parse_card_table(markdown: &str) -> Vec<CardRow> {
    let mut rows: Vec<CardRow> = Vec::new();
    let lines: Vec<&str> = markdown.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if is_table_line(lines[i]) {
            // Try to read a header + separator + ≥1 data line.
            if let Some(consumed) = try_read_table(&lines[i..], &mut rows) {
                i += consumed;
                continue;
            }
        }
        i += 1;
    }
    rows
}

fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() >= 3
}

/// Try to read a header line + separator line + ≥1 data line as a table.
/// Returns the number of lines consumed on success.
fn try_read_table(lines: &[&str], out: &mut Vec<CardRow>) -> Option<usize> {
    if lines.len() < 3 {
        return None;
    }
    let header_cells = split_cells(lines[0]);
    let sep_cells = split_cells(lines[1]);
    if !looks_like_separator(&sep_cells) {
        return None;
    }
    // Decide which columns we want.
    let benchmark_col = find_column(&header_cells, &["benchmark", "task", "dataset", "name"]);
    let score_col = find_numeric_column(&header_cells);
    let (Some(bcol), Some(scol)) = (benchmark_col, score_col) else {
        // No numeric column → skip the whole table (spec).
        return None;
    };
    let mut consumed = 2; // header + separator
    for line in &lines[2..] {
        if !is_table_line(line) {
            break;
        }
        let cells = split_cells(line);
        let Some(name) = cells.get(bcol).map(|s| s.trim().to_string()) else {
            consumed += 1;
            continue;
        };
        let Some(score_str) = cells.get(scol).map(|s| s.trim()) else {
            consumed += 1;
            continue;
        };
        let Ok(score) = score_str.parse::<f64>() else {
            consumed += 1;
            continue;
        };
        if name.is_empty() {
            consumed += 1;
            continue;
        }
        out.push(CardRow {
            benchmark: name,
            score,
        });
        consumed += 1;
    }
    Some(consumed)
}

fn split_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    // Drop leading/trailing '|' before splitting so we don't get empty
    // cells at both ends.
    let inner = trimmed
        .strip_prefix('|')
        .unwrap_or(trimmed)
        .strip_suffix('|')
        .unwrap_or(trimmed);
    inner.split('|').map(|s| s.trim().to_string()).collect()
}

fn looks_like_separator(cells: &[String]) -> bool {
    if cells.is_empty() {
        return false;
    }
    cells
        .iter()
        .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
}

fn find_column(headers: &[String], candidates: &[&str]) -> Option<usize> {
    headers.iter().position(|h| {
        let h = h.to_ascii_lowercase();
        candidates.iter().any(|c| h.contains(c))
    })
}

/// Decide which column carries numeric scores. Preference:
/// 1. A header explicitly naming a numeric quantity ("score", "value",
///    "accuracy", "acc", "metric", "result", anything containing "%").
/// 2. The first column whose separator-cell uses at least 3 dashes
///    (heuristic for "this is meant to hold a number").
fn find_numeric_column(headers: &[String]) -> Option<usize> {
    let priority = ["score", "value", "accuracy", "acc", "metric", "result"];
    for needle in &priority {
        if let Some(idx) = headers.iter().position(|h| {
            let h = h.to_ascii_lowercase();
            h == *needle || h.starts_with(needle)
        }) {
            return Some(idx);
        }
    }
    // Header containing '%' is a strong signal.
    if let Some(idx) = headers
        .iter()
        .position(|h| h.to_ascii_lowercase().contains('%'))
    {
        return Some(idx);
    }
    // Fallback: right-most column whose name looks like a number-bearer.
    if let Some(idx) = headers.iter().rposition(|h| {
        let h = h.to_ascii_lowercase();
        h.contains("score") || h.contains("value") || h.contains("metric")
    }) {
        return Some(idx);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_markdown_returns_empty() {
        assert!(parse_card_table("").is_empty());
    }

    #[test]
    fn markdown_without_tables_returns_empty() {
        let md = "## Results\n\nThis model is great.\n\n- bullet\n- another\n";
        assert!(parse_card_table(md).is_empty());
    }

    #[test]
    fn parses_simple_eval_table() {
        let md = "\
| Benchmark | Score |
|-----------|-------|
| MMLU      | 67.30 |
| GSM8K     | 52.10 |
";
        let rows = parse_card_table(md);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].benchmark, "MMLU");
        assert!((rows[0].score - 67.30).abs() < 1e-6);
        assert_eq!(rows[1].benchmark, "GSM8K");
        assert!((rows[1].score - 52.10).abs() < 1e-6);
    }
}