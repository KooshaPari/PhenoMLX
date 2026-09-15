//! Integration tests for [`hwledger_search_rag::StubEmbedder`].

use hwledger_search_rag::{Embedder, StubEmbedder};

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

#[test]
fn default_stub_embedder_has_dim_384() {
    let e = StubEmbedder::default();
    assert_eq!(e.dim(), 384);
    let v = e.embed("hello").expect("embed must succeed");
    assert_eq!(v.len(), 384);
}

#[test]
fn same_text_yields_identical_vector() {
    let e = StubEmbedder::default();
    let v1 = e.embed("hello world").unwrap();
    let v2 = e.embed("hello world").unwrap();
    assert_eq!(v1, v2, "deterministic embedder must yield identical vector");
}

#[test]
fn different_text_yields_different_vector() {
    let e = StubEmbedder::default();
    let v1 = e.embed("apples and oranges and fruit salad").unwrap();
    let v2 = e.embed("rust programming language systems software").unwrap();
    let sim = cosine(&v1, &v2);
    assert!(
        sim < 0.5,
        "cosine similarity for unrelated text should be < 0.5, got {sim}"
    );
}