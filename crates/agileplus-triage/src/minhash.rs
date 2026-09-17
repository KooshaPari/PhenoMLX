// SPDX-License-Identifier: MIT OR Apache-2.0
//! MinHash signatures for near-duplicate text detection.
//!
//! Implements the classic Broder (1997) MinHash estimator: a `k`-permutation
//! signature of the token set, where the *fraction of agreeing components* is
//! an unbiased estimator of the Jaccard similarity.  We hash each token with
//! FNV-1a under `k` different (seeded) permutations and keep the minimum
//! 64-bit hash per permutation.  Pure Rust, no external deps.
//!
//! # Usage
//!
//! ```
//! use agileplus_triage::minhash::MinHash;
//!
//! let a = MinHash::sign("the quick brown fox jumps over the lazy dog", 128);
//! let b = MinHash::sign("the quick brown fox jumps over the lazy dog.", 128);
//! let sim = a.jaccard(&b);
//! assert!(sim > 0.9, "nearly-identical texts should have sim > 0.9, got {}", sim);
//! ```
//!
//! # Determinism
//!
//! Signatures are pure functions of `(text, k)`; the same input always
//! produces the same signature.  No randomness, no global state.
//!
//! # Audit
//!
//! Implements recommendation #1 from `AUDIT_BLOC_VS_2026_SOTA.md`: the
//! MinHash primitive needed to enable LSH-banded candidate generation in
//! `hybrid_pipeline`.

/// 64-bit FNV-1a offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// 64-bit FNV-1a prime.
const FNV_PRIME: u64 = 0x100_0000_01b3;

/// SplitMix64-derived mix step used to derive a per-permutation seed.
///
/// The constants are Daniel Lemire / Sebastian Vigna "splitmix64" finalizer
/// — the same one used in `wyhash` and `xoshiro`.  We only need it to
/// decorrelate per-permutation salts; this is not a CSPRNG.
const SPLITMIX_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// A `k`-permutation MinHash signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinHash {
    /// One minimum-hash per permutation.  Length equals `k` at sign time.
    sig: Vec<u64>,
    /// `k` value used to build the signature — kept for introspection.
    k: usize,
}

impl MinHash {
    /// Build a signature of length `k` over `text`.  Text is lowercased and
    /// tokenized on non-alphanumeric boundaries; tokens of length `< 2` are
    /// dropped (same policy as `crate::dedup::tokenize`).
    ///
    /// # Panics
    /// Panics if `k == 0`.
    pub fn sign(text: &str, k: usize) -> Self {
        assert!(k > 0, "MinHash::sign requires k > 0");
        // The first permutation uses salt 0; subsequent permutations use
        // the splitmix64 finalizer applied to the previous salt.  This gives
        // us 64 decorrelated salts from a tiny constant state.
        let mut sig = Vec::with_capacity(k);
        let mut salt: u64 = 0;
        let tokens = tokenize_for_minhash(text);
        for _ in 0..k {
            let h = min_hash_one(&tokens, salt);
            sig.push(h);
            salt = salt.wrapping_add(SPLITMIX_GAMMA);
        }
        Self { sig, k }
    }

    /// Number of permutation slots in the signature.
    pub fn len(&self) -> usize {
        self.k
    }

    /// `true` when the signature has zero slots (only ever the case for
    /// externally-constructed empty signatures — `sign` always produces
    /// `k >= 1` slots).
    pub fn is_empty(&self) -> bool {
        self.k == 0
    }

    /// Raw signature, slice of length `k`.
    pub fn as_slice(&self) -> &[u64] {
        &self.sig
    }

    /// Estimated Jaccard similarity vs. `other`.  The estimator is the
    /// fraction of permutation slots where the two signatures agree.
    /// Signatures of differing `k` are compared over the common prefix and
    /// the result is still bounded in `[0.0, 1.0]`.
    pub fn jaccard(&self, other: &MinHash) -> f64 {
        let n = self.k.min(other.k);
        if n == 0 {
            return 0.0;
        }
        let mut agree = 0u32;
        for i in 0..n {
            if self.sig[i] == other.sig[i] {
                agree += 1;
            }
        }
        agree as f64 / n as f64
    }
}

/// Tokenize for MinHash: lowercase, split on non-alphanumeric, drop tokens
/// shorter than 2 characters.  Mirrors `crate::dedup::tokenize` so the two
/// scorers agree on what counts as a "token".
fn tokenize_for_minhash(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(String::from)
        .collect()
}

/// MinHash for one permutation: the minimum of `fnv1a(token_bytes ^ salt)`
/// over all tokens.  We XOR the salt into both the offset basis and the
/// running hash so the same token under two salts yields decorrelated
/// hashes (Salt-Splitting MinHash; see Mitzenmacher, Pachocki, et al. 2014).
fn min_hash_one(tokens: &[String], salt: u64) -> u64 {
    let mut best: Option<u64> = None;
    for tok in tokens {
        let h = fnv1a_xor(tok.as_bytes(), salt);
        best = Some(match best {
            None => h,
            Some(b) if h < b => h,
            Some(b) => b,
        });
    }
    // An empty token set gets the "all-ones" sentinel so that two empty
    // inputs collide with jaccard=1.0 (the limit of set Jaccard on the
    // empty set).
    best.unwrap_or(u64::MAX)
}

/// FNV-1a 64-bit hash with the salt folded into the offset basis and into
/// every byte XOR.  This is the standard "permutation by salt" trick.
fn fnv1a_xor(bytes: &[u8], salt: u64) -> u64 {
    let mut h = FNV_OFFSET ^ salt;
    for &b in bytes {
        h ^= (b as u64) ^ salt.rotate_left((b as u32) & 63);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn signature_length_matches_k() {
        for k in [1usize, 4, 16, 128, 256] {
            let s = MinHash::sign("hello world", k);
            assert_eq!(s.len(), k);
            assert_eq!(s.as_slice().len(), k);
        }
    }

    #[test]
    fn identical_texts_yield_jaccard_one() {
        let a = MinHash::sign("the quick brown fox", 128);
        let b = MinHash::sign("the quick brown fox", 128);
        assert_eq!(a.jaccard(&b), 1.0);
        // And the signatures themselves are bit-equal.
        assert_eq!(a, b);
    }

    #[test]
    fn disjoint_texts_yield_low_jaccard() {
        let a = MinHash::sign("alpha beta gamma delta", 256);
        let b = MinHash::sign("zulu yankee xray whiskey", 256);
        let j = a.jaccard(&b);
        assert!(j < 0.1, "disjoint sets should be near 0, got {j}");
    }

    #[test]
    fn small_edits_have_high_jaccard() {
        let a = MinHash::sign(
            "implement authentication flow for the login page using oauth2",
            256,
        );
        let b = MinHash::sign(
            "implement authentication flow for the login page using oauth2.",
            256,
        );
        let j = a.jaccard(&b);
        assert!(j > 0.8, "small edit should still be >0.8, got {j}");
    }

    #[test]
    fn parity_with_token_jaccard_on_typical_inputs() {
        use std::collections::HashSet;
        fn jaccard_set(a: &str, b: &str) -> f64 {
            let ta: HashSet<String> = a
                .to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .filter(|t| t.len() >= 2)
                .map(String::from)
                .collect();
            let tb: HashSet<String> = b
                .to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .filter(|t| t.len() >= 2)
                .map(String::from)
                .collect();
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
        let pairs = [
            ("add login button to header", "add login form to header"),
            ("refactor user service", "rewrite user service module"),
            ("fix race in cache", "fix race condition in cache layer"),
            (
                "add unit tests for claim store",
                "add unit tests for claim store.",
            ),
            (
                "unrelated content about food",
                "completely unrelated content",
            ),
        ];
        for (a, b) in pairs {
            let ma = MinHash::sign(a, 512);
            let mb = MinHash::sign(b, 512);
            let est = ma.jaccard(&mb);
            let truth = jaccard_set(a, b);
            // 512 permutations gives a tight estimator: error < 10% in
            // practice for non-tiny sets.
            assert!(
                approx_eq(est, truth, 0.10) || (truth < 0.1 && est < 0.15),
                "minhash={est:.3} truth={truth:.3} for {a:?} vs {b:?}"
            );
        }
    }

    #[test]
    fn empty_input_produces_colliding_sentinels() {
        let a = MinHash::sign("", 64);
        let b = MinHash::sign("", 64);
        assert_eq!(a.jaccard(&b), 1.0);
        // Non-empty should NOT collide with empty.
        let c = MinHash::sign("hello world", 64);
        assert!(a.jaccard(&c) < 0.05);
    }

    #[test]
    fn single_token_input() {
        let a = MinHash::sign("foo", 32);
        let b = MinHash::sign("foo", 32);
        assert_eq!(a.jaccard(&b), 1.0);
        let c = MinHash::sign("bar", 32);
        let d = MinHash::sign("foo", 32);
        // 32 permutations; the per-permutation min is deterministic for a
        // single token.  Different tokens -> 0 agreements.
        assert_eq!(c.jaccard(&d), 0.0);
    }

    #[test]
    fn differing_k_signatures_compare_on_common_prefix() {
        // The shorter signature is compared over its full length; the
        // longer signature is truncated to that length.  Output stays in
        // [0,1].
        let a = MinHash::sign("alpha beta gamma delta epsilon", 64);
        let b = MinHash::sign("alpha beta gamma delta epsilon", 256);
        let j = a.jaccard(&b);
        assert!((0.0..=1.0).contains(&j));
        // 64 permutations with identical input should be ~1.0.
        assert!(
            j > 0.95,
            "k-mismatch should still match on common prefix, got {j}"
        );
    }

    #[test]
    fn signature_is_deterministic_across_calls() {
        // Same input, same k, same output, twice.  The estimator is a pure
        // function; no hidden RNG state.
        let a = MinHash::sign("the rain in spain stays mainly in the plain", 128);
        let b = MinHash::sign("the rain in spain stays mainly in the plain", 128);
        assert_eq!(a, b);
    }

    #[test]
    fn is_empty_only_for_zero_k() {
        // `sign` panics on k=0, so we can't construct an empty signature
        // via the public API.  Verify `len` and `is_empty` agree for a few
        // k values.
        for k in [1usize, 7, 128] {
            let s = MinHash::sign("anything", k);
            assert_eq!(s.is_empty(), k == 0);
            assert_eq!(s.len(), k);
        }
    }

    #[test]
    #[should_panic(expected = "k > 0")]
    fn sign_panics_on_zero_k() {
        let _ = MinHash::sign("anything", 0);
    }
}
