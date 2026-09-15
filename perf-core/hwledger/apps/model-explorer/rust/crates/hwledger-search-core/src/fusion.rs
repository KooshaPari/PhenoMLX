//! Reciprocal Rank Fusion (RRF) — the deduplication-aware rank-aggregator we
//! use to combine BM25 and semantic retrieval lists.
//!
//! See Cormack et al., "Reciprocal Rank Fusion outperforms Condorcet and
//! individual Rank-Learning Methods", 2009, for the original formulation.
//! We use the canonical constant `k = 60`.

use serde::{Deserialize, Serialize};

/// One fused result row that knows where it appeared in each underlying list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scored {
    /// `source::id` (see [`crate::source_adapter::CandidateId::key`]).
    pub id: String,
    /// 1-based rank in the BM25 list (`None` if the candidate was absent).
    pub bm25_rank: Option<usize>,
    /// 1-based rank in the semantic list (`None` if absent).
    pub semantic_rank: Option<usize>,
    /// Raw BM25 score (normalized before passing in is the caller's job).
    pub bm25_score: f32,
    /// Raw semantic score (e.g. cosine, in `[-1, 1]` or `[0, 1]`).
    pub semantic_score: f32,
}

impl Scored {
    /// 1-indexed reciprocal-rank sum as score. Note: this is *informational*;
    /// `rrf_fuse` already sorts by the same quantity.
    pub fn rrf_score(&self, k: usize) -> f32 {
        let k = k as f32;
        let mut s = 0.0_f32;
        if let Some(r) = self.bm25_rank {
            s += 1.0 / (k + r as f32);
        }
        if let Some(r) = self.semantic_rank {
            s += 1.0 / (k + r as f32);
        }
        s
    }
}

/// Combine two ranked lists using Reciprocal Rank Fusion with the canonical
/// `k = 60` constant.
///
/// Input lists are expected to be ordered by *descending relevance*, with the
/// most relevant item at index 0. We treat the input position as rank-1 for
/// the first element. Scores (`bm25_score`, `semantic_score`) are preserved
/// exactly as supplied; this routine does not normalize them.
///
/// Output is sorted by descending RRF score, ties broken by id ascending for
/// determinism, and truncated to the first `k` rows.
pub fn rrf_fuse(bm25: &[(String, f32)], semantic: &[(String, f32)], k: usize) -> Vec<Scored> {
    const RRF_K: usize = 60;

    // Index every (id -> entry) so the two passes can fold into one map.
    let mut map: std::collections::BTreeMap<String, Scored> =
        std::collections::BTreeMap::new();

    for (rank_zero, (id, score)) in bm25.iter().enumerate() {
        let entry = map.entry(id.clone()).or_insert_with(|| Scored {
            id: id.clone(),
            bm25_rank: None,
            semantic_rank: None,
            bm25_score: 0.0,
            semantic_score: 0.0,
        });
        entry.bm25_rank = Some(rank_zero + 1);
        entry.bm25_score = *score;
    }

    for (rank_zero, (id, score)) in semantic.iter().enumerate() {
        let entry = map.entry(id.clone()).or_insert_with(|| Scored {
            id: id.clone(),
            bm25_rank: None,
            semantic_rank: None,
            bm25_score: 0.0,
            semantic_score: 0.0,
        });
        entry.semantic_rank = Some(rank_zero + 1);
        entry.semantic_score = *score;
    }

    let mut out: Vec<Scored> = map.into_values().collect();
    out.sort_by(|a, b| {
        let sa = a.rrf_score(RRF_K);
        let sb = b.rrf_score(RRF_K);
        // Descending score, ascending id for deterministic ties.
        sb.partial_cmp(&sa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    out.truncate(k);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_score_sums_both_lists() {
        let s = Scored {
            id: "x".into(),
            bm25_rank: Some(1),
            semantic_rank: Some(2),
            bm25_score: 1.0,
            semantic_score: 1.0,
        };
        // 1/(60+1) + 1/(60+2) ≈ 0.01639 + 0.01613 ≈ 0.03252
        let got = s.rrf_score(60);
        assert!((got - (1.0 / 61.0 + 1.0 / 62.0)).abs() < 1e-6);
    }

    #[test]
    fn rrf_score_handles_missing_list() {
        let s = Scored {
            id: "x".into(),
            bm25_rank: Some(3),
            semantic_rank: None,
            bm25_score: 0.0,
            semantic_score: 0.0,
        };
        assert!((s.rrf_score(60) - (1.0 / 63.0)).abs() < 1e-6);
    }
}
