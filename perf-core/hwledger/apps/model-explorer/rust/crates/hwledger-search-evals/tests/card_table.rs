//! Integration tests for [`hwledger_search_evals::parse_card_table`].

use hwledger_search_evals::{parse_card_table, CardRow};

const MD_WITH_TABLE: &str = "\
# Some Model Card

This is an intro paragraph.

## Results

| Benchmark | Score |
|-----------|-------|
| MMLU      | 67.30 |
| GSM8K     | 52.10 |
| HumanEval |  35.0 |

## Notes

No tables here, just prose.
";

const MD_WITHOUT_TABLES: &str = "\
# Card

This is just prose.

- bullet one
- bullet two

No tables whatsoever.
";

#[test]
fn parses_single_eval_table() {
    let rows: Vec<CardRow> = parse_card_table(MD_WITH_TABLE);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].benchmark, "MMLU");
    assert!((rows[0].score - 67.30).abs() < 1e-6);
    assert_eq!(rows[1].benchmark, "GSM8K");
    assert!((rows[1].score - 52.10).abs() < 1e-6);
    assert_eq!(rows[2].benchmark, "HumanEval");
    assert!((rows[2].score - 35.0).abs() < 1e-6);
}

#[test]
fn markdown_without_tables_returns_empty_vec() {
    assert!(parse_card_table(MD_WITHOUT_TABLES).is_empty());
    assert!(parse_card_table("").is_empty());
}