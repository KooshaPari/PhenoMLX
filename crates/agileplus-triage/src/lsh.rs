// SPDX-License-Identifier: MIT OR Apache-2.0
//! LSH (Locality-Sensitive Hashing) banding for MinHash signatures.
//!
//! `LshIndex` partitions a MinHash signature into `num_bands` bands of
//! `rows_per_band` rows each. Two documents with Jaccard similarity `s`
//! have probability `1 - (1 - s^r)^b` of colliding in at least one band.
//!
//! Traceability: audit rec #13 (LSH banding for MinHash).

use std::collections::HashMap;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x100_0000_01b3;

/// LSH index over MinHash signatures.
#[derive(Debug, Clone, Default)]
pub struct LshIndex {
    bands: Vec<HashMap<u64, Vec<String>>>,
    num_bands: usize,
    rows_per_band: usize,
}

impl LshIndex {
    /// Create an empty index with the given banding parameters.
    pub fn new(num_bands: usize, rows_per_band: usize) -> Self {
        assert!(num_bands > 0, "num_bands must be > 0");
        assert!(rows_per_band > 0, "rows_per_band must be > 0");
        Self {
            bands: (0..num_bands).map(|_| HashMap::new()).collect(),
            num_bands,
            rows_per_band,
        }
    }

    pub fn num_bands(&self) -> usize {
        self.num_bands
    }
    pub fn rows_per_band(&self) -> usize {
        self.rows_per_band
    }
    pub fn expected_signature_len(&self) -> usize {
        self.num_bands * self.rows_per_band
    }

    /// Insert a document ID into all band hash buckets.
    pub fn insert(&mut self, id: &str, signature: &[u64]) {
        let expected = self.expected_signature_len();
        assert_eq!(
            signature.len(),
            expected,
            "signature length {} does not match num_bands * rows_per_band = {}",
            signature.len(),
            expected
        );
        for (band_idx, band) in self.bands.iter_mut().enumerate() {
            let band_start = band_idx * self.rows_per_band;
            let band_end = band_start + self.rows_per_band;
            let h = Self::band_hash(signature, band_start, band_end);
            let bucket = band.entry(h).or_default();
            if !bucket.contains(&id.to_string()) {
                bucket.push(id.to_string());
            }
        }
    }

    /// Query for candidate document IDs that share at least one band hash.
    pub fn query(&self, signature: &[u64]) -> Vec<String> {
        let expected = self.expected_signature_len();
        assert_eq!(
            signature.len(),
            expected,
            "signature length {} does not match num_bands * rows_per_band = {}",
            signature.len(),
            expected
        );
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for (band_idx, band) in self.bands.iter().enumerate() {
            let band_start = band_idx * self.rows_per_band;
            let band_end = band_start + self.rows_per_band;
            let h = Self::band_hash(signature, band_start, band_end);
            if let Some(bucket) = band.get(&h) {
                for id in bucket {
                    if seen.insert(id.clone()) {
                        out.push(id.clone());
                    }
                }
            }
        }
        out
    }

    /// FNV-1a hash of a band slice.
    pub fn band_hash(signature: &[u64], band_start: usize, band_end: usize) -> u64 {
        assert!(band_start <= band_end, "band_start must be <= band_end");
        assert!(
            band_end <= signature.len(),
            "band_end must be <= signature.len()"
        );
        let mut h = FNV_OFFSET;
        for &v in &signature[band_start..band_end] {
            h ^= v;
            h = h.wrapping_mul(FNV_PRIME);
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_sig(seed: u64, len: usize) -> Vec<u64> {
        (0..len).map(|i| seed.wrapping_add(i as u64)).collect()
    }

    #[test]
    fn lsh_index_new_parameters() {
        let idx = LshIndex::new(4, 8);
        assert_eq!(idx.num_bands(), 4);
        assert_eq!(idx.rows_per_band(), 8);
        assert_eq!(idx.expected_signature_len(), 32);
    }

    #[test]
    fn lsh_insert_and_query_finds_same_doc() {
        let mut idx = LshIndex::new(5, 4);
        let sig = dummy_sig(42, 20);
        idx.insert("doc-a", &sig);
        let candidates = idx.query(&sig);
        assert!(candidates.contains(&"doc-a".to_string()));
    }

    #[test]
    fn lsh_query_returns_empty_for_no_matches() {
        let idx = LshIndex::new(5, 4);
        let sig = dummy_sig(99, 20);
        let candidates = idx.query(&sig);
        assert!(candidates.is_empty());
    }

    #[test]
    fn lsh_similar_signatures_collide() {
        let mut idx = LshIndex::new(10, 5);
        let sig_a = dummy_sig(0, 50);
        let mut sig_b = sig_a.clone();
        sig_b[0] = 9999;
        idx.insert("doc-a", &sig_a);
        let candidates = idx.query(&sig_b);
        assert!(candidates.contains(&"doc-a".to_string()));
    }

    #[test]
    fn lsh_dissimilar_signatures_rarely_collide() {
        let mut idx = LshIndex::new(10, 5);
        let sig_a = dummy_sig(0, 50);
        let sig_b = dummy_sig(1000, 50);
        idx.insert("doc-a", &sig_a);
        let candidates = idx.query(&sig_b);
        assert!(
            candidates.is_empty(),
            "dissimilar signatures should not collide"
        );
    }

    #[test]
    fn lsh_insert_deduplicates_duplicate_ids() {
        let mut idx = LshIndex::new(3, 4);
        let sig = dummy_sig(1, 12);
        idx.insert("dup", &sig);
        idx.insert("dup", &sig);
        let candidates = idx.query(&sig);
        assert_eq!(
            candidates.iter().filter(|id| id.as_str() == "dup").count(),
            1
        );
    }

    #[test]
    fn lsh_band_hash_is_deterministic() {
        let sig = vec![1u64, 2, 3, 4, 5];
        let h1 = LshIndex::band_hash(&sig, 0, 3);
        let h2 = LshIndex::band_hash(&sig, 0, 3);
        assert_eq!(h1, h2);
    }

    #[test]
    fn lsh_band_hash_changes_with_slice() {
        let sig = vec![1u64, 2, 3, 4, 5];
        let h1 = LshIndex::band_hash(&sig, 0, 3);
        let h2 = LshIndex::band_hash(&sig, 1, 4);
        assert_ne!(h1, h2);
    }

    #[test]
    fn lsh_band_hash_empty_band() {
        let sig = vec![1u64, 2, 3];
        let h = LshIndex::band_hash(&sig, 0, 0);
        assert_eq!(h, FNV_OFFSET);
    }

    #[test]
    #[should_panic(expected = "num_bands must be > 0")]
    fn lsh_new_panics_on_zero_bands() {
        let _ = LshIndex::new(0, 5);
    }

    #[test]
    #[should_panic(expected = "rows_per_band must be > 0")]
    fn lsh_new_panics_on_zero_rows() {
        let _ = LshIndex::new(5, 0);
    }

    #[test]
    #[should_panic(expected = "signature length")]
    fn lsh_insert_panics_on_wrong_signature_len() {
        let mut idx = LshIndex::new(5, 4);
        let sig = dummy_sig(0, 19);
        idx.insert("x", &sig);
    }

    #[test]
    fn lsh_query_returns_multiple_candidates() {
        let mut idx = LshIndex::new(2, 4);
        let sig1 = dummy_sig(0, 8);
        let mut sig2 = sig1.clone();
        sig2[4] = 999;
        idx.insert("doc-1", &sig1);
        idx.insert("doc-2", &sig2);
        let candidates = idx.query(&sig1);
        assert!(candidates.contains(&"doc-1".to_string()));
        assert!(candidates.contains(&"doc-2".to_string()));
    }
}
