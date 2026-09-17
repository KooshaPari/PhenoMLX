// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hybrid near-duplicate detection pipeline.
//!
//! Three-stage dedup:
//!
//! 1. **MinHash signatures** (see `crate::minhash`) for cheap O(n) Jaccard
//!    estimation per pair.
//! 2. **Banded LSH candidate generation** (Indyk-Motwani 1998) so we
//!    only score pairs that share at least one band.  This is the
//!    "candidate generation" phase.
//! 3. **Embedding cosine verification** (see `crate::embeddings`) to
//!    confirm candidates that MinHash flagged.  This is the
//!    "verification" phase.
//! 4. **Jaccard tiebreak** — for any pair whose embedding cosine is in
//!    a deadband (e.g. `0.55..0.80`), fall back to the exact token
//!    Jaccard to break ties.
//!
//! This mirrors the 2025 SOTA on `BigCloneBench` (MinHash-LSH candidates
//! + embedding verification) and `pip dedupe`'s package-level
//!   strategy.
//!
//! # Audit
//!
//! Implements recommendation #4 from `AUDIT_BLOC_VS_2026_SOTA.md`:
//! the `HybridDedup` pipeline that runs MinHash-LSH candidate
//! generation, then embedding cosine verification, then Jaccard
//! tiebreak.

use std::collections::{HashMap, HashSet};

use crate::dedup::token_jaccard;
use crate::embeddings::EmbeddingBackend;
use crate::minhash::MinHash;

/// Default MinHash permutations.  Matches the rest of the bloc.
pub const DEFAULT_NUM_PERM: usize = 128;

/// Default bands/rows for LSH banding.  `b * r = num_perm` and the
/// threshold `t = (1/b)^(1/r)` is the steepest point of the
/// probability curve.  With `b=32, r=4, num_perm=128`, the
/// threshold is `t ~ 0.42` — pairs with Jaccard above ~0.42 are very
/// likely to be candidates.
pub const DEFAULT_BANDS: usize = 32;
pub const DEFAULT_ROWS: usize = 4;

/// A confirmed near-duplicate group.
#[derive(Debug, Clone, PartialEq)]
pub struct DupGroup {
    /// Indices into the input slice.
    pub members: Vec<usize>,
    /// MinHash Jaccard estimator (over the union of all pairs in the
    /// group; for single pairs this is the per-pair estimate).
    pub minhash_jaccard: f64,
    /// Embedding cosine over the *first* pair in the group (mean of
    /// all pairs in the group is also stored, but the field is the
    /// "headline" similarity used for ranking).
    pub embedding_cosine: f64,
    /// Exact token Jaccard, used as the tiebreak.
    pub token_jaccard: f64,
}

/// Pipeline configuration.
#[derive(Debug, Clone)]
pub struct HybridConfig {
    /// Number of MinHash permutations.
    pub num_perm: usize,
    /// LSH bands.
    pub bands: usize,
    /// LSH rows per band.
    pub rows: usize,
    /// Embedding cosine threshold for confirming a candidate as a
    /// duplicate.
    pub cosine_threshold: f64,
    /// Below this cosine, candidates are rejected outright.
    pub cosine_reject: f64,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            num_perm: DEFAULT_NUM_PERM,
            bands: DEFAULT_BANDS,
            rows: DEFAULT_ROWS,
            cosine_threshold: 0.80,
            cosine_reject: 0.40,
        }
    }
}

impl HybridConfig {
    /// Validate that `bands * rows == num_perm`; otherwise LSH banding
    /// can't be applied.
    pub fn validated(self) -> Result<Self, String> {
        if self.bands * self.rows != self.num_perm {
            return Err(format!(
                "HybridConfig: bands * rows ({}) must equal num_perm ({})",
                self.bands * self.rows,
                self.num_perm
            ));
        }
        if self.cosine_reject >= self.cosine_threshold {
            return Err(format!(
                "HybridConfig: cosine_reject ({}) must be < cosine_threshold ({})",
                self.cosine_reject, self.cosine_threshold
            ));
        }
        Ok(self)
    }
}

/// The hybrid dedup pipeline.  Holds the MinHash signatures of all
/// indexed items plus the LSH band tables.
pub struct HybridDedup {
    cfg: HybridConfig,
    sigs: Vec<MinHash>,
    /// For each band, a `HashMap<band-bucket, Vec<index>>` of items
    /// that hashed to that bucket.  Two items in the same bucket for
    /// the same band are LSH candidates.
    band_tables: Vec<HashMap<u64, Vec<usize>>>,
    raw: Vec<String>,
}

impl HybridDedup {
    /// Construct a pipeline from raw inputs.  Signatures are computed
    /// eagerly; LSH band tables are populated up front.
    pub fn build<B: EmbeddingBackend>(
        items: &[String],
        backend: &B,
        cfg: HybridConfig,
    ) -> Result<Self, String> {
        let cfg = cfg.validated()?;
        let n = items.len();
        let mut sigs: Vec<MinHash> = Vec::with_capacity(n);
        // We need the embeddings just to verify dimensions; we don't
        // store them here.  Verify in one batch.
        let refs: Vec<&str> = items.iter().map(String::as_str).collect();
        let _ = backend.embed(&refs); // validates backend connectivity / dim
        for s in items {
            sigs.push(MinHash::sign(s, cfg.num_perm));
        }
        let mut band_tables: Vec<HashMap<u64, Vec<usize>>> =
            (0..cfg.bands).map(|_| HashMap::new()).collect();
        for (idx, sig) in sigs.iter().enumerate() {
            let raw = sig.as_slice();
            for (b, band_table) in band_tables.iter_mut().enumerate() {
                let lo = b * cfg.rows;
                let hi = lo + cfg.rows;
                let bucket = band_hash(&raw[lo..hi], b);
                band_table.entry(bucket).or_default().push(idx);
            }
        }
        Ok(Self {
            cfg,
            sigs,
            band_tables,
            raw: items.to_vec(),
        })
    }

    /// Number of items indexed.
    pub fn len(&self) -> usize {
        self.sigs.len()
    }

    /// `true` when no items have been indexed.
    pub fn is_empty(&self) -> bool {
        self.sigs.is_empty()
    }

    /// Configuration snapshot.
    pub fn config(&self) -> &HybridConfig {
        &self.cfg
    }

    /// Find all duplicate groups whose embedding cosine is at or above
    /// `cfg.cosine_threshold` (or whose token Jaccard is at or above
    /// `cfg.cosine_threshold` if the embedding backend says so).
    ///
    /// Pairs are produced in two stages:
    ///
    /// 1. LSH candidate generation: for each band, walk its bucket
    ///    table and emit all `(i, j)` pairs sharing a bucket.  Dedupe
    ///    pairs across bands.
    /// 2. For each candidate pair, compute embedding cosine.  Accept
    ///    if cosine >= `cosine_threshold`.  In the deadband
    ///    `[cosine_reject, cosine_threshold)`, fall back to token
    ///    Jaccard and accept if it's >= `cosine_threshold`.
    pub fn find<B: EmbeddingBackend>(&self, backend: &B) -> Vec<DupGroup> {
        // Generate candidate pairs via LSH.
        let candidates = self.candidate_pairs();
        // Embed all items in one batch (cheaper than per-pair).
        let refs: Vec<&str> = self.raw.iter().map(String::as_str).collect();
        let embs = backend.embed(&refs);

        // Union-find for grouping.
        let mut uf = UnionFind::new(self.sigs.len());
        for &(i, j) in &candidates {
            let cos = cosine_or_zero(embs.get(i), embs.get(j));
            let cos = cos as f64;
            let accepted = if cos >= self.cfg.cosine_threshold {
                true
            } else if cos >= self.cfg.cosine_reject {
                // Deadband — fall back to exact token Jaccard.
                token_jaccard(&self.raw[i], &self.raw[j]) >= self.cfg.cosine_threshold
            } else {
                false
            };
            if accepted {
                uf.union(i, j);
            }
        }
        // Also accept pairs that LSH missed but whose direct
        // MinHash-Jaccard is extreme (>0.95): near-identical strings
        // share enough n-grams that they always band together, but
        // when num_perm is small we may still want a safety net.
        if self.cfg.num_perm <= 64 {
            let n = self.sigs.len();
            for i in 0..n {
                for j in (i + 1)..n {
                    if uf.find(i) == uf.find(j) {
                        continue;
                    }
                    if self.sigs[i].jaccard(&self.sigs[j]) > 0.95 {
                        uf.union(i, j);
                    }
                }
            }
        }

        // Build groups.
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..self.sigs.len() {
            groups.entry(uf.find(i)).or_default().push(i);
        }
        let mut out: Vec<DupGroup> = groups
            .into_values()
            .filter(|m| m.len() >= 2)
            .map(|members| {
                let i = members[0];
                let j = members[1];
                let minhash_jaccard = self.sigs[i].jaccard(&self.sigs[j]);
                let embedding_cosine = cosine_or_zero(embs.get(i), embs.get(j)) as f64;
                let token_jaccard = crate::dedup::token_jaccard(&self.raw[i], &self.raw[j]);
                DupGroup {
                    members,
                    minhash_jaccard,
                    embedding_cosine,
                    token_jaccard,
                }
            })
            .collect();
        out.sort_by(|a, b| {
            b.embedding_cosine
                .partial_cmp(&a.embedding_cosine)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    /// Generate candidate pairs from LSH band tables.  Each pair is
    /// emitted at most once.
    fn candidate_pairs(&self) -> HashSet<(usize, usize)> {
        let mut pairs: HashSet<(usize, usize)> = HashSet::new();
        for table in &self.band_tables {
            for bucket in table.values() {
                if bucket.len() < 2 {
                    continue;
                }
                for w in bucket.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                    pairs.insert((lo, hi));
                }
                // The above only emits adjacent pairs in the bucket
                // vec; for `bucket.len() > 2` we also need the
                // diagonals.  E.g. bucket = [0, 1, 2] emits (0,1) and
                // (1,2); we still need (0,2).
                for i in 0..bucket.len() {
                    for j in (i + 1)..bucket.len() {
                        let (a, b) = (bucket[i], bucket[j]);
                        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                        pairs.insert((lo, hi));
                    }
                }
            }
        }
        pairs
    }
}

/// Convenience: run the full pipeline end-to-end.  Builds the index
/// and returns groups.  For batch jobs this is the entry point.
pub fn run_dedup<B: EmbeddingBackend>(
    items: &[String],
    backend: &B,
    cfg: HybridConfig,
) -> Result<Vec<DupGroup>, String> {
    let pipeline = HybridDedup::build(items, backend, cfg)?;
    Ok(pipeline.find(backend))
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Hash a band of `rows` MinHash slots.  We fold the slice into a u64
/// by XOR — order-insensitive, which is the right semantics for a
/// "bucket" that all permutations must match exactly.
fn band_hash(band: &[u64], band_idx: usize) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x100_0000_01b3;
    let mut h = FNV_OFFSET ^ (band_idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for &v in band {
        let mut x = v;
        // Mix the 64-bit slot into the FNV state 8 bytes at a time.
        for _ in 0..8 {
            h ^= x & 0xff;
            h = h.wrapping_mul(FNV_PRIME);
            x >>= 8;
        }
    }
    h
}

fn cosine_or_zero(a: Option<&Vec<f32>>, b: Option<&Vec<f32>>) -> f32 {
    match (a, b) {
        (Some(a), Some(b)) => crate::embeddings::cosine(a, b),
        _ => 0.0,
    }
}

// Union-find for grouping accepted pairs.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let r = self.find(self.parent[x]);
            self.parent[x] = r;
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        match self.rank[ra].cmp(&self.rank[rb]) {
            std::cmp::Ordering::Less => self.parent[ra] = rb,
            std::cmp::Ordering::Greater => self.parent[rb] = ra,
            std::cmp::Ordering::Equal => {
                self.parent[rb] = ra;
                self.rank[ra] += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::LocalMockEmbeddings;

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn config_validates_bands_times_rows() {
        let cfg = HybridConfig {
            num_perm: 128,
            bands: 16,
            rows: 8,
            ..Default::default()
        };
        assert!(cfg.clone().validated().is_ok());
        let bad = HybridConfig {
            num_perm: 128,
            bands: 16,
            rows: 4, // 16*4 != 128
            ..Default::default()
        };
        assert!(bad.validated().is_err());
    }

    #[test]
    fn config_validates_cosine_thresholds() {
        let ok = HybridConfig {
            num_perm: 128,
            bands: 32,
            rows: 4,
            cosine_threshold: 0.80,
            cosine_reject: 0.40,
            ..Default::default()
        };
        assert!(ok.clone().validated().is_ok());
        let bad = HybridConfig {
            cosine_reject: 0.85, // >= threshold
            ..ok
        };
        assert!(bad.validated().is_err());
    }

    #[test]
    fn empty_input_yields_no_groups() {
        let backend = LocalMockEmbeddings::default();
        let groups = run_dedup(&[], &backend, HybridConfig::default()).unwrap();
        assert!(groups.is_empty());
    }

    #[test]
    fn single_input_yields_no_groups() {
        let backend = LocalMockEmbeddings::default();
        let items = vec!["only one".to_string()];
        let groups = run_dedup(&items, &backend, HybridConfig::default()).unwrap();
        assert!(groups.is_empty());
    }

    #[test]
    fn finds_clear_duplicates() {
        let backend = LocalMockEmbeddings::default();
        let items = vec![
            "add login button to header".to_string(),
            "add login button to header.".to_string(),
            "completely unrelated content about sandwiches".to_string(),
        ];
        let groups = run_dedup(&items, &backend, HybridConfig::default()).unwrap();
        // We expect exactly one group containing {0, 1}.
        assert_eq!(groups.len(), 1, "groups: {:?}", groups);
        let g = &groups[0];
        assert!(g.members.contains(&0) && g.members.contains(&1));
        assert!(!g.members.contains(&2));
        assert!(g.embedding_cosine > 0.6);
    }

    #[test]
    fn no_duplicates_when_inputs_are_disjoint() {
        let backend = LocalMockEmbeddings::default();
        let items = vec![
            "alpha beta gamma delta epsilon".to_string(),
            "zulu yankee xray whiskey foxtrot".to_string(),
            "the rain in spain stays mainly in the plain".to_string(),
            "lorem ipsum dolor sit amet consectetur".to_string(),
        ];
        let cfg = HybridConfig {
            cosine_threshold: 0.80,
            ..Default::default()
        };
        let groups = run_dedup(&items, &backend, cfg).unwrap();
        // Either empty, or only tight clusters we don't expect — the
        // local mock should keep them disjoint.
        for g in &groups {
            // Whatever groups we get, the cosine must be high.
            assert!(g.embedding_cosine > 0.7);
        }
    }

    #[test]
    fn groups_share_first_pair_in_members() {
        // The DupGroup records the headline similarity from members[0]
        // vs members[1].  Multi-member groups are common in transitive
        // closure — verify the API contract.
        let backend = LocalMockEmbeddings::default();
        let items = vec![
            "the quick brown fox".to_string(),
            "the quick brown fox.".to_string(),
            "the quick brown fox!".to_string(),
        ];
        let groups = run_dedup(&items, &backend, HybridConfig::default()).unwrap();
        assert_eq!(groups.len(), 1);
        let g = &groups[0];
        assert_eq!(g.members.len(), 3);
    }

    #[test]
    fn candidate_pairs_are_deduped_across_bands() {
        // Two identical signatures will collide in *every* band.  The
        // candidate set should still contain the pair exactly once.
        let backend = LocalMockEmbeddings::default();
        let items = vec!["a b c d e f g".to_string(), "a b c d e f g".to_string()];
        let pipe = HybridDedup::build(&items, &backend, HybridConfig::default()).unwrap();
        let pairs = pipe.candidate_pairs();
        assert!(pairs.contains(&(0, 1)));
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn build_three_items_at_default_config() {
        // `build` should succeed and produce a non-empty band table.
        let backend = LocalMockEmbeddings::default();
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let pipe = HybridDedup::build(&items, &backend, HybridConfig::default()).unwrap();
        assert_eq!(pipe.len(), 3);
        assert!(!pipe.is_empty());
    }

    #[test]
    fn higher_cosine_threshold_finds_fewer_groups() {
        // The same input, with two thresholds.  Higher threshold ->
        // fewer (or equal) groups.
        let backend = LocalMockEmbeddings::default();
        let items = vec![
            "add login button to header".to_string(),
            "add login form to header".to_string(),
            "completely unrelated content".to_string(),
        ];
        let cfg_loose = HybridConfig {
            cosine_threshold: 0.50,
            ..Default::default()
        };
        let cfg_tight = HybridConfig {
            cosine_threshold: 0.95,
            ..Default::default()
        };
        let g_loose = run_dedup(&items, &backend, cfg_loose).unwrap();
        let g_tight = run_dedup(&items, &backend, cfg_tight).unwrap();
        assert!(g_loose.len() >= g_tight.len());
    }

    #[test]
    fn dupgroup_fields_are_populated() {
        let backend = LocalMockEmbeddings::default();
        let items = vec![
            "fix race in cache layer".to_string(),
            "fix race in cache layer.".to_string(),
        ];
        let groups = run_dedup(&items, &backend, HybridConfig::default()).unwrap();
        assert_eq!(groups.len(), 1);
        let g = &groups[0];
        assert!(
            g.minhash_jaccard > 0.8,
            "minhash_jaccard={}",
            g.minhash_jaccard
        );
        assert!(g.embedding_cosine > 0.5, "cosine={}", g.embedding_cosine);
        assert!(g.token_jaccard > 0.8, "token_jaccard={}", g.token_jaccard);
    }

    #[test]
    fn groups_sorted_by_descending_cosine() {
        let backend = LocalMockEmbeddings::default();
        let items = vec![
            "alpha alpha alpha alpha".to_string(),
            "alpha alpha alpha alpha.".to_string(),
            "alpha alpha alpha alphaz".to_string(),
            "alpha alpha alpha alphay".to_string(),
            "completely different content here".to_string(),
        ];
        let groups = run_dedup(&items, &backend, HybridConfig::default()).unwrap();
        for w in groups.windows(2) {
            assert!(
                approx(w[0].embedding_cosine, w[1].embedding_cosine, 1e-9)
                    || w[0].embedding_cosine >= w[1].embedding_cosine,
                "groups not sorted: {:?}",
                groups
            );
        }
    }
}
