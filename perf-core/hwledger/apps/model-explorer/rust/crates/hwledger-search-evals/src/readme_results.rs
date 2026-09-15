//! README prose extractor — pulls inline benchmark scores from the
//! "Results" / "Benchmark" / "Evaluation" sections of a model card.
//!
//! The regex we use:
//! ```text
//! (?i)\b(MMLU|HumanEval|GSM8K|BBH|HellaSwag|ARC|WinoGrande|
//!        TruthfulQA|MATH|IFEval|MT-Bench|ChatbotArena|AlpacaEval)\b
//! [^.\n]{0,80}?
//! (\d+\.?\d*)
//! ```
//! captures the benchmark name and the first number that follows it
//! within ~80 chars (no crossing newlines or sentence boundaries).
//!
//! [`parse_readme_results`] returns the highest score per benchmark.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// One benchmark result extracted from inline README prose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadmeEval {
    /// Canonical benchmark name as it appeared in the prose.
    pub benchmark: String,
    /// Numeric score.
    pub score: f64,
}

impl Default for ReadmeEval {
    fn default() -> Self {
        Self {
            benchmark: String::new(),
            score: 0.0,
        }
    }
}

/// The bench-name regex alternation, broken out so we can also use it for
/// header sniffing (`"## MMLU Results"` should still count).
const BENCH_NAMES: &str = "MMLU|HumanEval|GSM8K|BBH|HellaSwag|ARC|WinoGrande|TruthfulQA|MATH|IFEval|MT-Bench|ChatbotArena|AlpacaEval";

static RESULT_REGEX: OnceLock<Regex> = OnceLock::new();

fn result_regex() -> &'static Regex {
    RESULT_REGEX.get_or_init(|| {
        // The capture groups are: 1 = benchmark name, 2 = score.
        Regex::new(&format!(
            r"(?i)\b({BENCH_NAMES})\b[^\.\n]{{0,80}}?(\d+\.?\d*)"
        ))
        .expect("result regex must compile")
    })
}

static HEADER_REGEX: OnceLock<Regex> = OnceLock::new();

fn header_regex() -> &'static Regex {
    HEADER_REGEX.get_or_init(|| {
        // ATX-style heading (## … ######) or Setext-style (=== / ---) that
        // mentions one of Results / Benchmark / Evaluation.
        Regex::new(r"(?im)^\s{0,3}#{1,6}\s+(?i)(Results|Benchmark|Evaluation).*$|^[^\n]*\n[=\-]{3,}\s*$")
            .expect("header regex must compile")
    })
}

/// Scan `markdown` and return one [`ReadmeEval`] per benchmark found in
/// a "Results" / "Benchmark" / "Evaluation" section header. If no such
/// header is present, the whole document is scanned. Duplicates are
/// collapsed to the highest score.
pub fn parse_readme_results(markdown: &str) -> Vec<ReadmeEval> {
    if markdown.trim().is_empty() {
        return Vec::new();
    }
    let section = extract_relevant_section(markdown);
    let mut best: HashMap<String, f64> = HashMap::new();
    for cap in result_regex().captures_iter(section) {
        let Some(name_match) = cap.get(1) else { continue };
        let Some(num_match) = cap.get(2) else { continue };
        let name = normalize_name(name_match.as_str());
        let Ok(score) = num_match.as_str().parse::<f64>() else { continue };
        let entry = best.entry(name).or_insert(f64::NEG_INFINITY);
        if score > *entry {
            *entry = score;
        }
    }
    let mut out: Vec<ReadmeEval> = best
        .into_iter()
        .map(|(benchmark, score)| ReadmeEval { benchmark, score })
        .collect();
    out.sort_by(|a, b| a.benchmark.cmp(&b.benchmark));
    out
}

/// Restrict the scan to the first "Results / Benchmark / Evaluation"
/// section if one exists, otherwise return the whole markdown.
fn extract_relevant_section(markdown: &str) -> &str {
    let Some(m) = header_regex().find(markdown) else {
        return markdown;
    };
    // Walk forward to find the next ATX heading (start of next section)
    // or Setext underline.
    let after = &markdown[m.end()..];
    let bytes = after.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Look for the next '#' that begins a line and is followed by
        // a space — that's the next ATX heading.
        if bytes[i] == b'#' {
            // Check the start of this potential heading.
            let line_start = after[..i]
                .rfind('\n')
                .map(|p| p + 1)
                .unwrap_or(0);
            let rest = &after[line_start..];
            if rest.starts_with('#') {
                // Must be '#' followed by either '#' or whitespace, not
                // mid-word.
                let mut j = 0;
                while j < rest.len() && rest.as_bytes()[j] == b'#' {
                    j += 1;
                }
                if j < rest.len() && rest.as_bytes()[j] == b' ' {
                    return &after[..line_start];
                }
            }
        }
        i += 1;
    }
    after
}

/// Canonicalize the benchmark name so equivalent variants ("MMLU",
/// "mmlu", "MMLU ") collapse together.
fn normalize_name(s: &str) -> String {
    let trimmed = s.trim();
    // Camel-case the first letter so "MMLU" / "Mmlu" / "mmlu" all match.
    let mut chars: Vec<char> = trimmed.chars().collect();
    if let Some(first) = chars.first_mut() {
        *first = first.to_ascii_uppercase();
    }
    for c in chars.iter_mut().skip(1) {
        *c = c.to_ascii_lowercase();
    }
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_markdown_returns_empty() {
        assert!(parse_readme_results("").is_empty());
    }

    #[test]
    fn extracts_simple_prose_score() {
        let md = "## Results\n\nMMLU: 67.30\n";
        let out = parse_readme_results(md);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].benchmark, "Mmlu");
        assert!((out[0].score - 67.30).abs() < 1e-6);
    }

    #[test]
    fn dedupes_keeping_highest_score() {
        let md = "\
## Results
MMLU: 60.0
Some other text mentioning MMLU at 65.0 somewhere
GSM8K: 52.10
MMLU: 70.5
";
        let out = parse_readme_results(md);
        let mmlu = out.iter().find(|r| r.benchmark == "Mmlu").expect("MMLU row");
        assert!((mmlu.score - 70.5).abs() < 1e-6);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn restricts_to_results_section() {
        let md = "\
# Intro
MMLU: 99.0

## Results
MMLU: 60.0
GSM8K: 52.0
";
        let out = parse_readme_results(md);
        let mmlu = out.iter().find(|r| r.benchmark == "Mmlu").expect("MMLU row");
        assert!((mmlu.score - 60.0).abs() < 1e-6, "section-restricted score");
        assert_eq!(out.len(), 2);
    }
}