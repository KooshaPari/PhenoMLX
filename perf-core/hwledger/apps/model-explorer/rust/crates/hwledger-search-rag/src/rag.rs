//! Cosine-similarity retrieval over a flat in-memory chunk collection.
//!
//! [`retrieve`] embeds the query, re-embeds each candidate [`Chunk`] on
//! the fly through the same [`Embedder`], and returns the top-`k` chunks
//! ranked by cosine similarity. Embeddings are *not* cached: the trait
//! surface is intentionally side-effect free so the same call works
//! against any backend.
//!
//! For real workloads the chunker → embedder → vector index pipeline
//! lives in `hwledger-search-index`; this module is the offline, no-IO
//! reference implementation used by tests and the CLI's `--local` mode.

use serde::{Deserialize, Serialize};

use crate::chunker::Chunk;
use crate::embedder::Embedder;
use crate::error::RagError;

/// One retrieval hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RagResult {
    /// 1-based rank within the returned top-`k` (1 = best).
    pub rank: u32,
    /// Cosine similarity score in `[-1, 1]`.
    pub score: f32,
    /// Section label inherited from the source chunk.
    pub section: String,
    /// Chunk text.
    pub text: String,
}

impl Default for RagResult {
    fn default() -> Self {
        Self {
            rank: 0,
            score: 0.0,
            section: String::new(),
            text: String::new(),
        }
    }
}

/// Knobs for [`retrieve`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RAGConfig {
    /// How many hits to return.
    pub top_k: usize,
}

impl Default for RAGConfig {
    fn default() -> Self {
        Self { top_k: 8 }
    }
}

/// Embed `query`, score every [`Chunk`] against it via cosine similarity,
/// and return the top-`k` hits in descending score order.
///
/// Returns an empty `Vec` if `chunks` is empty. Returns
/// [`RagError::EmptyQuery`] for a whitespace-only query and
/// [`RagError::DimMismatch`] if any per-chunk embedding has a different
/// dimensionality than the query embedding.
pub async fn retrieve(
    embedder: &dyn Embedder,
    query: &str,
    chunks: &[Chunk],
    top_k: usize,
) -> Result<Vec<RagResult>, RagError> {
    if query.trim().is_empty() {
        return Err(RagError::EmptyQuery);
    }
    if chunks.is_empty() || top_k == 0 {
        return Ok(Vec::new());
    }

    let query_vec = embedder.embed(query)?;
    if query_vec.len() != embedder.dim() {
        return Err(RagError::DimMismatch {
            expected: embedder.dim(),
            actual: query_vec.len(),
        });
    }
    let query_norm = l2_norm(&query_vec);

    let mut scored: Vec<(usize, RagResult)> = Vec::with_capacity(chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        let vec = embedder.embed(&chunk.text)?;
        if vec.len() != query_vec.len() {
            return Err(RagError::DimMismatch {
                expected: query_vec.len(),
                actual: vec.len(),
            });
        }
        let norm = l2_norm(&vec);
        let score = if norm == 0.0 || query_norm == 0.0 {
            0.0
        } else {
            dot(&query_vec, &vec) / (query_norm * norm)
        };
        scored.push((
            i,
            RagResult {
                rank: 0, // filled in after sort
                score,
                section: chunk.section.clone(),
                text: chunk.text.clone(),
            },
        ));
    }

    // Sort by score desc; tie-break by original chunk index for stability.
    scored.sort_by(|a, b| {
        b.1.score
            .partial_cmp(&a.1.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    let take = top_k.min(scored.len());
    let mut out: Vec<RagResult> = Vec::with_capacity(take);
    for (rank_idx, (_, mut r)) in scored.into_iter().take(take).enumerate() {
        r.rank = (rank_idx as u32) + 1;
        out.push(r);
    }
    Ok(out)
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedder::StubEmbedder;

    #[tokio::test]
    async fn empty_chunks_returns_empty() {
        let e = StubEmbedder::default();
        let r = retrieve(&e, "hello", &[], 5).await.unwrap();
        assert!(r.is_empty());
    }

    #[tokio::test]
    async fn empty_query_errors() {
        let e = StubEmbedder::default();
        let chunks: Vec<Chunk> = vec![Chunk {
            index: 0,
            section: "card".into(),
            text: "hi".into(),
            token_offset: 0,
        }];
        let err = retrieve(&e, "   ", &chunks, 5).await.unwrap_err();
        assert!(matches!(err, RagError::EmptyQuery));
    }

    #[tokio::test]
    async fn results_sorted_descending() {
        let e = StubEmbedder::default();
        let chunks: Vec<Chunk> = ["alpha", "beta", "gamma", "delta"]
            .iter()
            .enumerate()
            .map(|(i, t)| Chunk {
                index: i as u32,
                section: "card".into(),
                text: (*t).to_string(),
                token_offset: 0,
            })
            .collect();
        let r = retrieve(&e, "alpha", &chunks, 4).await.unwrap();
        assert_eq!(r.len(), 4);
        assert_eq!(r[0].rank, 1);
        assert_eq!(r[0].text, "alpha"); // same text → cosine = 1.0
        for w in r.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
    }
}