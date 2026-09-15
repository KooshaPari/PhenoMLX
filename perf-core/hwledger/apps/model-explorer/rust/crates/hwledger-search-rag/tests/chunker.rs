//! Integration tests for [`hwledger_search_rag::Chunker`].

use hwledger_search_rag::{Chunk, Chunker};

#[test]
fn default_chunker_returns_at_least_one_chunk_for_nonempty_text() {
    let body = "This is a small piece of model-card prose.";
    let chunks = Chunker::default().chunk(body, "card");
    assert!(!chunks.is_empty(), "expected >=1 chunk");
    for c in &chunks {
        assert_eq!(c.section, "card");
        assert!(!c.text.is_empty(), "chunk text must not be empty");
    }
}

#[test]
fn chunked_text_covers_full_input() {
    // Small text that fits a single window → concatenation equals the input
    // exactly (paragraph-aware path).
    let body = "hello world\n\nsecond paragraph here\n\nthird paragraph";
    let chunks = Chunker::default().chunk(body, "card");
    assert!(!chunks.is_empty());
    let joined: String = chunks.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join("\n\n");
    assert_eq!(joined, body);

    // Larger text: no character of the input may be lost across the
    // concatenation (paragraphs keep their original boundaries).
    let paragraphs: Vec<&str> = (0..20)
        .map(|i| match i % 3 {
            0 => "alpha paragraph with some words for chunking purposes",
            1 => "beta paragraph that is just a tiny bit longer than alpha",
            _ => "gamma",
        })
        .collect();
    let big = paragraphs.join("\n\n");
    let chunks = Chunker::default().chunk(&big, "card");
    let joined = chunks
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    assert_eq!(joined, big, "small-paragraph input must be losslessly chunked");
}

#[test]
fn chunks_have_increasing_index_and_nonempty_text() {
    let body = (0..30)
        .map(|i| format!("paragraph number {i} with some content"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let chunks: Vec<Chunk> = Chunker::default().chunk(&body, "card");
    assert!(chunks.len() >= 2, "expected multiple chunks from 30 paragraphs");
    let mut last_index: u32 = 0;
    for c in &chunks {
        assert!(
            !c.text.trim().is_empty(),
            "chunk text must be non-empty: index={}",
            c.index
        );
        assert!(
            c.index >= last_index,
            "chunk index must be monotonically non-decreasing"
        );
        last_index = c.index;
    }
    // And the sequence must be strictly increasing at least until we hit
    // the final chunk — chunker emits consecutive indices starting at 0.
    for w in chunks.windows(2) {
        assert_eq!(w[1].index, w[0].index + 1);
    }
}