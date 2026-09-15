//! Integration tests for [`hwledger_search_evals::parse_readme_results`].

use hwledger_search_evals::{parse_readme_results, ReadmeEval};

#[test]
fn extracts_prose_benchmark_score() {
    let md = "## Results\n\nOn MMLU we report 67.30, and on GSM8K we reach 52.10.\n";
    let out: Vec<ReadmeEval> = parse_readme_results(md);
    assert_eq!(out.len(), 2);
    let names: Vec<&str> = out.iter().map(|r| r.benchmark.as_str()).collect();
    assert!(names.contains(&"Mmlu"), "expected Mmlu, got {names:?}");
    assert!(names.contains(&"Gsm8k"), "expected Gsm8k, got {names:?}");
    let mmlu = out.iter().find(|r| r.benchmark == "Mmlu").unwrap();
    assert!((mmlu.score - 67.30).abs() < 1e-6);
}

#[test]
fn empty_input_returns_empty_vec() {
    assert!(parse_readme_results("").is_empty());
    assert!(parse_readme_results("   \n\n").is_empty());
}

#[test]
fn dedupes_keeping_highest_score() {
    let md = "## Results\n\nMMLU: 60.0\n\nThen later MMLU: 75.5\n";
    let out = parse_readme_results(md);
    assert_eq!(out.len(), 1);
    assert!((out[0].score - 75.5).abs() < 1e-6);
}