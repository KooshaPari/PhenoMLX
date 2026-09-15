//! Integration tests for the variant-collapse logic.
//!
//! These hit the public [`collapse_variants`], [`collapse_key`], and
//! [`CollapseRule`] surface directly — no Tantivy involved.

use hwledger_search_index::{collapse_key, collapse_variants, CollapseRule, IndexHit};

fn hit(id: &str, score: f32) -> IndexHit {
    IndexHit::new(id, score)
}

#[test]
fn collapse_groups_a_variants_and_keeps_b_separate() {
    let rule = CollapseRule::default();
    let hits = vec![
        hit("a-Q4_K_M", 1.0),
        hit("a-Q5_K_M", 0.9),
        hit("a-Q8_0", 0.8),
        hit("b", 0.5),
    ];
    let groups = collapse_variants(hits, &rule);
    assert_eq!(groups.len(), 2, "expected two collapsed groups");

    // The `a` group must come first (highest top-hit score) and contain
    // all three a-* variants in input order.
    assert_eq!(groups[0].base_id, "a");
    assert_eq!(
        groups[0].variants,
        vec!["a-Q4_K_M", "a-Q5_K_M", "a-Q8_0"],
        "variants must be preserved in input order"
    );
    assert!((groups[0].top_hit.score - 1.0).abs() < 1e-6);

    // The `b` row has no quant suffix and stays in its own group.
    assert_eq!(groups[1].base_id, "b");
    assert_eq!(groups[1].variants, vec!["b"]);
}

#[test]
fn collapse_respects_preserve_provenance() {
    // A finetune marker (`-ft`) must keep the row in its own group even when
    // the base id matches.
    let rule = CollapseRule::default().with_preserve_provenance(vec!["-ft"]);
    let hits = vec![hit("a-Q4_K_M", 1.0), hit("a-ft-Q4_K_M", 0.5)];
    let groups = collapse_variants(hits, &rule);
    assert_eq!(
        groups.len(),
        2,
        "finetune-marked row must not collapse with its base"
    );

    let ids: Vec<&str> = groups.iter().map(|g| g.base_id.as_str()).collect();
    // The provenance-marked row strips to `a-ft`; the unmarked one to `a`.
    assert!(
        ids.contains(&"a"),
        "expected the unmarked base `a` in groups, got {:?}",
        ids
    );
    assert!(
        ids.contains(&"a-ft"),
        "expected the provenance-marked base `a-ft` in groups, got {:?}",
        ids
    );

    // Neither group should contain ids from the other.
    for g in &groups {
        if g.base_id == "a" {
            assert_eq!(g.variants, vec!["a-Q4_K_M"]);
        } else {
            assert_eq!(g.variants, vec!["a-ft-Q4_K_M"]);
        }
    }
}

#[test]
fn collapse_key_strips_the_first_matching_suffix_only() {
    let rule = CollapseRule::default();

    // Plain case — single match.
    assert_eq!(collapse_key("a-Q4_K_M", &rule), "a");

    // Two suffix candidates in the same id; only the right-most one is
    // removed. With case-insensitive matching we can build this with mixed
    // separators.
    let double = "a-Q4_K_M.Q8_0";
    assert_eq!(
        collapse_key(double, &rule),
        "a-Q4_K_M",
        "only the right-most suffix should be stripped"
    );

    // Non-matching suffix leaves the id alone.
    assert_eq!(collapse_key("plain", &rule), "plain");
    assert_eq!(collapse_key("a-FOO", &rule), "a-FOO");

    // Empty suffix in the rule is ignored (defensive).
    let noisy = CollapseRule {
        collapse_quant_suffixes: vec!["".into(), ".q4_k_m".into()],
        preserve_provenance: vec![],
    };
    assert_eq!(collapse_key("a.q4_k_m", &noisy), "a");
}