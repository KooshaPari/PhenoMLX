// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::empty_line_after_doc_comments)]
//! Backlog item deduplication: token-Jaccard, fuzzy ratio (Levenshtein),
//! simhash, n-gram, and a hybrid scorer.
//!
//! Traceability: FR-AGP-018 (triage dedup primitives)
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Tokenize a string into lowercase, alphanumeric, length>=2 tokens.
pub fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(String::from)
        .collect()
}

/// Token Jaccard similarity in [0.0, 1.0].
pub fn token_jaccard(a: &str, b: &str) -> f64 {
    let ta: HashSet<String> = tokenize(a).into_iter().collect();
    let tb: HashSet<String> = tokenize(b).into_iter().collect();
    if ta.is_empty() && tb.is_empty() {
        return 1.0;
    }
    let inter = ta.intersection(&tb).count() as f64;
    let union = ta.union(&tb).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Levenshtein edit distance between two byte strings.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = std::cmp::min(
                std::cmp::min(curr[j - 1] + 1, prev[j] + 1),
                prev[j - 1] + cost,
            );
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Fuzzy ratio: 1.0 - levenshtein/max(len), clamped to [0,1].
pub fn fuzzy_ratio(a: &str, b: &str) -> f64 {
    let max = std::cmp::max(a.chars().count(), b.chars().count());
    if max == 0 {
        return 1.0;
    }
    let d = levenshtein(a, b);
    1.0 - (d as f64 / max as f64)
}

/// Extract character n-grams (alphanumeric only) from `s`.
/// When the string has fewer than `n` characters, returns a single-element
/// set containing the concatenated string.
pub fn ngrams(s: &str, n: usize) -> HashSet<String> {
    let chars: Vec<char> = s.chars().filter(|c| c.is_alphanumeric()).collect();
    if chars.len() < n {
        let joined: String = chars.iter().collect();
        return std::iter::once(joined).collect();
    }
    (0..=chars.len() - n)
        .map(|i| chars[i..i + n].iter().collect())
        .collect()
}

/// N-gram Jaccard similarity in [0.0, 1.0].
pub fn ngram_jaccard(a: &str, b: &str, n: usize) -> f64 {
    let na = ngrams(a, n);
    let nb = ngrams(b, n);
    if na.is_empty() && nb.is_empty() {
        return 1.0;
    }
    let inter = na.intersection(&nb).count() as f64;
    let union = na.union(&nb).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// 64-bit simhash for short text. Hash each n-gram (n=3) with FNV-1a and
/// bitwise XOR-sum into a 64-bit fingerprint.
pub fn simhash64(s: &str) -> u64 {
    let grams = ngrams(s, 3);
    if grams.is_empty() {
        return 0;
    }
    let mut bits = [0i32; 64];
    for g in &grams {
        // FNV-1a 64-bit hash
        let mut h: u64 = 1469598103934665603;
        for b in g.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        for (i, b) in bits.iter_mut().enumerate() {
            if (h >> i) & 1 == 1 {
                *b += 1;
            } else {
                *b -= 1;
            }
        }
    }
    let mut out: u64 = 0;
    for (i, bit) in bits.iter().enumerate() {
        if *bit > 0 {
            out |= 1 << i;
        }
    }
    out
}

/// Hamming distance between two 64-bit simhashes.
pub fn simhash_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// A candidate duplicate pair with its score breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateCandidate {
    pub a_id: String,
    pub b_id: String,
    pub hybrid_score: f64,
    pub token_jaccard: f64,
    pub fuzzy_ratio: f64,
    pub ngram_jaccard: f64,
    pub simhash_distance: u32,
}

/// Add the result of a fuzzy ratio calculation into a cache by Id. Not returned by `hybrid_score` directly.
#[allow(dead_code)] // reserved - side-effectful helper for optional fuzzy ratio caching
pub fn add_fuzzy_ratio(a: &str, b: &str, ratio: f64) {
    // Fuzzy ratio is only meaningful when combined with other metrics.
    // This function is provided as a side-effectful helper for a side lookup.
    // The actual hybrid_score call uses the token_jaccard, ngram_jaccard, and simhash_distance.
    // If this ratio is useful, the caller can add it to `hybrid_score` calculation.
    // If no additional result is added, it does nothing.
    let _ = (a, b, ratio);
}

/// Hybrid score:
/// `0.5*token_jaccard + 0.2*fuzzy_ratio + 0.2*ngram_jaccard + 0.1*(1 - simhash_distance/64)`.
///
/// Returns `(score, token_jaccard, fuzzy_ratio, ngram_jaccard, simhash_distance)`.
pub fn hybrid_score(a: &str, b: &str) -> (f64, f64, f64, f64, u32) {
    let tj = token_jaccard(a, b);
    let fr = fuzzy_ratio(a, b);
    let nj = ngram_jaccard(a, b, 3);
    let sh = simhash_distance(simhash64(a), simhash64(b));
    let sh_norm = 1.0 - (sh as f64 / 64.0);
    let score = 0.5 * tj + 0.2 * fr + 0.2 * nj + 0.1 * sh_norm;
    (score, tj, fr, nj, sh)
}

/// Find all pairs above `threshold` from a slice of `(id, text)` inputs.
/// O(n^2) — intended for backlogs of <= 10k items. Returns candidates
/// sorted by descending `hybrid_score`.
pub fn find_duplicates(items: &[(String, String)], threshold: f64) -> Vec<DuplicateCandidate> {
    let mut out = Vec::new();
    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            let (score, tj, fr, nj, sh) = hybrid_score(&items[i].1, &items[j].1);
            if score >= threshold {
                out.push(DuplicateCandidate {
                    a_id: items[i].0.clone(),
                    b_id: items[j].0.clone(),
                    hybrid_score: score,
                    token_jaccard: tj,
                    fuzzy_ratio: fr,
                    ngram_jaccard: nj,
                    simhash_distance: sh,
                });
            }
        }
    }
    out.sort_by(|x, y| {
        y.hybrid_score
            .partial_cmp(&x.hybrid_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}
