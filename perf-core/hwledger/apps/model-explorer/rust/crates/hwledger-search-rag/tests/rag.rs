//! Integration tests for [`hwledger_search_rag::retrieve`].

use hwledger_search_rag::{retrieve, Chunk, StubEmbedder};

fn fixture_chunks() -> Vec<Chunk> {
    [
        "the cat sat on the mat",
        "rust is a systems programming language",
        "apples and oranges are tasty fruits",
        "neural networks learn from gradients",
        "the dog played in the park",
    ]
    .iter()
    .enumerate()
    .map(|(i, t)| Chunk {
        index: i as u32,
        section: "card".into(),
        text: (*t).to_string(),
        token_offset: 0,
    })
    .collect()
}

#[tokio::test]
async fn retrieve_returns_top_k_sorted_descending_by_score() {
    let embedder = StubEmbedder::default();
    let chunks = fixture_chunks();
    let top_k = 3;
    let results = retrieve(&embedder, "rust programming language", &chunks, top_k)
        .await
        .expect("retrieve must succeed");
    assert_eq!(results.len(), top_k);
    assert_eq!(results[0].rank, 1);
    assert_eq!(results[1].rank, 2);
    assert_eq!(results[2].rank, 3);
    for w in results.windows(2) {
        assert!(
            w[0].score >= w[1].score,
            "results must be sorted desc by score: {} vs {}",
            w[0].score,
            w[1].score
        );
    }
    // Every returned hit must have a finite cosine score in [-1, 1] and
    // its text must come from the fixture.
    for r in &results {
        assert!(r.score.is_finite(), "score must be finite: {}", r.score);
        assert!(
            (-1.0..=1.0).contains(&r.score),
            "score must be in [-1,1]: {}",
            r.score
        );
        assert!(
            chunks.iter().any(|c| c.text == r.text),
            "result text {:?} not in input chunks",
            r.text
        );
    }
}

#[tokio::test]
async fn retrieve_with_empty_chunks_returns_empty() {
    let embedder = StubEmbedder::default();
    let results = retrieve(&embedder, "anything", &[], 5)
        .await
        .expect("empty input must not error");
    assert!(results.is_empty());
}