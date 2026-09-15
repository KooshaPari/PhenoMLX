//! Reciprocal Rank Fusion tests.

use hwledger_search_core::rrf_fuse;

#[test]
fn overlapping_top_ranks_make_one_winner() {
    // Both lists agree on "a", differ on "b" vs "c".
    let bm25 = vec![
        ("a".to_string(), 5.0),
        ("b".to_string(), 3.0),
        ("d".to_string(), 1.0),
    ];
    let sem = vec![
        ("a".to_string(), 0.9),
        ("c".to_string(), 0.7),
        ("d".to_string(), 0.4),
    ];
    let fused = rrf_fuse(&bm25, &sem, 10);

    // "a" appears in both lists at rank 1 — it must come out on top.
    assert!(!fused.is_empty());
    assert_eq!(fused[0].id, "a");
    // And it carries both rank fields.
    assert_eq!(fused[0].bm25_rank, Some(1));
    assert_eq!(fused[0].semantic_rank, Some(1));

    // All four ids should be present.
    let ids: std::collections::BTreeSet<_> = fused.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains("a"));
    assert!(ids.contains("b"));
    assert!(ids.contains("c"));
    assert!(ids.contains("d"));
}

#[test]
fn disjoint_lists_still_fuse_to_all_items() {
    let bm25 = vec![("a".to_string(), 1.0), ("b".to_string(), 0.5)];
    let sem = vec![("c".to_string(), 1.0), ("d".to_string(), 0.5)];
    let fused = rrf_fuse(&bm25, &sem, 10);

    let ids: std::collections::BTreeSet<_> = fused.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids.len(), 4);
    assert!(ids.contains("a"));
    assert!(ids.contains("b"));
    assert!(ids.contains("c"));
    assert!(ids.contains("d"));

    // Disjoint: each item only has its source-list rank populated.
    for s in &fused {
        assert!(
            s.bm25_rank.is_some() ^ s.semantic_rank.is_some(),
            "{s:?} should have exactly one rank set",
        );
    }
}

#[test]
fn limit_k_truncates_output() {
    let bm25: Vec<_> = (0..10)
        .map(|i| (format!("id_{i}"), 10.0 - i as f32))
        .collect();
    let sem: Vec<_> = (0..10)
        .map(|i| (format!("id_{i}"), 1.0 - i as f32 * 0.1))
        .collect();

    let fused = rrf_fuse(&bm25, &sem, 3);
    assert_eq!(fused.len(), 3);

    // Output must be sorted by descending RRF score (id_0 in both at rank 1
    // is the strongest single candidate).
    for pair in fused.windows(2) {
        let a = pair[0].rrf_score(60);
        let b = pair[1].rrf_score(60);
        assert!(a >= b, "scores must be non-increasing: {a} < {b}");
    }
}
