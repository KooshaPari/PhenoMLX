//! Integration tests for [`hwledger_search_evals::parse_model_index`].

use hwledger_search_evals::{parse_model_index, EvalRecord};

const SAMPLE_YAML: &str = "\
model-index:
- name: llm-leaderboard-eval
  results:
  - task: {type: knowledge, name: mmlu}
    dataset:
      name: hendrycksTest
      type: text-only
    metrics:
    - name: acc
      value: 67.30
    source:
      url: https://huggingface.co/spaces/HuggingFaceH4/open_llm_leaderboard
";

#[test]
fn parses_hendrycks_mmlu_record() {
    let recs = parse_model_index(SAMPLE_YAML);
    assert_eq!(recs.len(), 1, "expected exactly one EvalRecord");
    let r: &EvalRecord = &recs[0];
    assert_eq!(r.benchmark, "hendrycksTest/mmlu");
    assert!(
        (r.score - 67.30).abs() < 1e-6,
        "score should be 67.30, got {}",
        r.score
    );
    assert_eq!(
        r.source_url.as_deref(),
        Some("https://huggingface.co/spaces/HuggingFaceH4/open_llm_leaderboard"),
    );
}

#[test]
fn empty_input_returns_empty_vec() {
    assert!(parse_model_index("").is_empty());
    assert!(parse_model_index("   \n\n# not yaml\n").is_empty());
}