// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bloom filter for high-throughput "have I seen this before?" membership
//! tests at the issue-intake layer.
//!
//! A Bloom filter is a fixed-size bit vector with `k` independent hash
//! functions.  Insertion sets the `k` bits indexed by the hash functions;
//! membership queries return `true` iff **all** `k` bits are set.  False
//! positives are possible (a `true` answer for a non-inserted item) but
//! false negatives are not.  The false-positive rate `p` for `n` inserted
//! items and `m` bits is approximately `(1 - e^(-kn/m))^k`, minimized at
//! `k = (m/n) * ln(2)`, giving `m = -n ln(p) / (ln 2)^2` and
//! `k = -log2(p)`.
//!
//! # Hashing
//!
//! We use FNV-1a + xxhash-style double hashing: two independent 64-bit
//! hashes `h1`, `h2` per key, and the `i`-th slot is
//! `h1.wrapping_add((i as u64).wrapping_mul(h2)) % m_bits`.  This is the
//! same construction `fastbloom` uses; it gives us `k` independent hashes
//! from just two base hashes, with no need to seed `k` independent
//! permutation functions.
//!
//! # Audit
//!
//! Implements recommendation #2 from `AUDIT_BLOC_VS_2026_SOTA.md`: the
//! Bloom filter primitive for issue-intake "have I seen this hash?".
//!
//! # Feature gate
//!
//! This module is gated behind `#[cfg(feature = "bloom")]` (the bitvec
//! backing store).  The default `agileplus-triage` build enables `bloom`.

#[cfg(feature = "bloom")]
use bitvec::prelude::*;

/// Compute the bit-vector size `m` for a target false-positive rate `p`
/// and an expected number of inserted items `n`.
///
/// `m = ceil(-n * ln(p) / ln(2)^2)`, rounded up to the next power of two
/// for cheap modulo.  Returns `0` if the inputs are degenerate
/// (`n == 0`, `p` outside `(0, 1)`).
pub fn optimal_m(n: usize, p: f64) -> usize {
    if n == 0 || p <= 0.0 || p >= 1.0 || !p.is_finite() {
        return 0;
    }
    let m = (-((n as f64) * p.ln()) / (2f64.ln().powi(2))).ceil() as usize;
    // Round up to a power of two for `m % m_bits` to compile to a bitand.
    let mut pow2 = 1usize;
    while pow2 < m {
        pow2 = pow2.saturating_mul(2);
    }
    // Cap at a sane upper bound to avoid pathological inputs allocating
    // gibibytes of bits.  2^28 bits = 32 MiB.
    pow2.min(1usize << 28)
}

/// Compute the optimal number of hash slots `k` for a target FP rate `p`.
///
/// `k = ceil(-log2(p))`, clamped to `[1, 16]`.
pub fn optimal_k(p: f64) -> usize {
    if !(p > 0.0 && p < 1.0) {
        return 1;
    }
    let k = (-p.log2()).ceil() as usize;
    k.clamp(1, 16)
}

/// A Bloom filter with a target false-positive rate and expected
/// cardinality.  Storage is a `bitvec` of `m` bits; `k` independent hash
/// slots are derived from two base hashes (FNV-1a + a second FNV-1a pass
/// with a different offset).
#[cfg(feature = "bloom")]
#[derive(Debug, Clone)]
pub struct BloomFilter {
    bits: BitVec,
    m: usize,
    k: usize,
    /// Number of items successfully inserted (caller-provided bound,
    /// incremented on `insert`).
    n_inserted: usize,
    /// Target FP rate at construction.
    target_fp: f64,
}

#[cfg(feature = "bloom")]
impl BloomFilter {
    /// Construct a filter sized for `expected_items` items at
    /// `target_fp` false-positive rate.  `target_fp` is clamped to
    /// `[1e-6, 0.5]`.
    ///
    /// # Panics
    /// Panics if `expected_items == 0` or `target_fp` is non-finite.
    pub fn new(expected_items: usize, target_fp: f64) -> Self {
        assert!(expected_items > 0, "expected_items must be > 0");
        assert!(target_fp.is_finite(), "target_fp must be finite");
        let p = target_fp.clamp(1e-6, 0.5);
        let m = optimal_m(expected_items, p);
        let k = optimal_k(p);
        Self {
            bits: bitvec![0; m],
            m,
            k,
            n_inserted: 0,
            target_fp: p,
        }
    }

    /// Construct a filter for 10,000 items at 1% false-positive rate.
    pub fn with_defaults() -> Self {
        Self::new(10_000, 0.01)
    }

    /// Bit-vector length (number of hash slots).
    pub fn m(&self) -> usize {
        self.m
    }

    /// Number of hash functions per lookup.
    pub fn k(&self) -> usize {
        self.k
    }

    /// Items inserted so far.
    pub fn len(&self) -> usize {
        self.n_inserted
    }

    /// `true` iff no items have been inserted.
    pub fn is_empty(&self) -> bool {
        self.n_inserted == 0
    }

    /// Target FP rate at construction.
    pub fn target_fp(&self) -> f64 {
        self.target_fp
    }

    /// Number of `1` bits currently set.  Saturates to `m`.
    pub fn popcount(&self) -> usize {
        self.bits.count_ones()
    }

    /// Empirical false-positive estimate based on saturation:
    /// `1 - (1 - popcount/m)^k`.  Useful for the test suite.
    pub fn empirical_fp(&self) -> f64 {
        if self.m == 0 {
            return 0.0;
        }
        let p = self.bits.count_ones() as f64 / self.m as f64;
        1.0 - (1.0 - p).powi(self.k as i32)
    }

    /// Insert a byte slice.  Always succeeds.
    pub fn insert(&mut self, item: &[u8]) {
        let (h1, h2) = double_hash(item);
        for i in 0..self.k {
            let idx = self.index(h1, h2, i);
            // Safety: `self.index` is masked to `[0, self.m)`.
            self.bits
                .get_mut(idx)
                .expect("bloom: index out of range")
                .set(true);
        }
        self.n_inserted += 1;
    }

    /// Test membership.  May return `true` for items that were never
    /// inserted (false positive); never returns `false` for inserted items.
    pub fn contains(&self, item: &[u8]) -> bool {
        let (h1, h2) = double_hash(item);
        for i in 0..self.k {
            let idx = self.index(h1, h2, i);
            if !self.bits.get(idx).map(|b| *b).unwrap_or(false) {
                return false;
            }
        }
        true
    }

    /// Bit index for the `i`-th hash slot.
    #[inline]
    fn index(&self, h1: u64, h2: u64, i: usize) -> usize {
        // Double-hashing: g_i(x) = h1(x) + i * h2(x).  Mask to `m` via
        // `m & (m - 1) == 0` since `m` is a power of two.
        (h1.wrapping_add((i as u64).wrapping_mul(h2)) & (self.m as u64 - 1)) as usize
    }

    /// Clear all bits and reset the inserted counter.
    pub fn clear(&mut self) {
        self.bits.fill(false);
        self.n_inserted = 0;
    }
}

#[cfg(feature = "bloom")]
fn fnv1a(bytes: &[u8], offset_xor: u64) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x100_0000_01b3;
    let mut h = FNV_OFFSET ^ offset_xor;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

#[cfg(feature = "bloom")]
fn double_hash(bytes: &[u8]) -> (u64, u64) {
    let h1 = fnv1a(bytes, 0);
    let h2 = fnv1a(bytes, 0x9E37_79B9_7F4A_7C15);
    (h1, h2)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------- helpers not gated by `bloom` --------

    #[test]
    fn optimal_m_grows_with_n() {
        let m1 = optimal_m(100, 0.01);
        let m2 = optimal_m(10_000, 0.01);
        assert!(m2 > m1, "m should grow with n: {} vs {}", m1, m2);
    }

    #[test]
    fn optimal_m_grows_as_fp_shrinks() {
        let m_loose = optimal_m(10_000, 0.1);
        let m_tight = optimal_m(10_000, 0.001);
        assert!(m_tight > m_loose);
    }

    #[test]
    fn optimal_m_is_power_of_two() {
        for (n, p) in [(100, 0.01), (10_000, 0.001), (50_000, 0.05), (1, 0.5)] {
            let m = optimal_m(n, p);
            assert!(m > 0, "m must be positive for n={},p={}", n, p);
            assert_eq!(m & (m - 1), 0, "m={} is not a power of two", m);
        }
    }

    #[test]
    fn optimal_m_zero_for_degenerate_inputs() {
        assert_eq!(optimal_m(0, 0.01), 0);
        assert_eq!(optimal_m(100, 0.0), 0);
        assert_eq!(optimal_m(100, 1.0), 0);
        assert_eq!(optimal_m(100, f64::NAN), 0);
    }

    #[test]
    fn optimal_k_is_minus_log2_p() {
        assert_eq!(optimal_k(0.5), 1);
        assert_eq!(optimal_k(0.25), 2);
        assert_eq!(optimal_k(0.01), 7); // ceil(-log2 0.01) = ceil(6.64) = 7
        assert!(optimal_k(0.001) >= 10);
    }

    #[test]
    fn optimal_k_clamps_extremes() {
        // p very close to 0 -> k clamped to 16
        assert_eq!(optimal_k(1e-12), 16);
        // p degenerate -> 1
        assert_eq!(optimal_k(0.0), 1);
        assert_eq!(optimal_k(1.0), 1);
        assert_eq!(optimal_k(2.0), 1);
    }

    // -------- feature-gated tests --------

    #[cfg(feature = "bloom")]
    fn rnd_bytes(i: usize) -> Vec<u8> {
        // Deterministic, fast, no external RNG.  We just need a
        // well-spread set of keys for the FP rate test.
        let mut s = i as u64;
        let mut out = Vec::with_capacity(16);
        for _ in 0..16 {
            // splitmix64 step
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            out.extend_from_slice(&z.to_le_bytes());
        }
        out
    }

    #[cfg(feature = "bloom")]
    #[test]
    fn contains_returns_true_after_insert() {
        let mut bf = BloomFilter::new(1_000, 0.01);
        let key: &[u8] = b"hello world";
        assert!(!bf.contains(key));
        bf.insert(key);
        assert!(bf.contains(key));
        assert_eq!(bf.len(), 1);
    }

    #[cfg(feature = "bloom")]
    #[test]
    fn contains_returns_false_for_unrelated_keys() {
        // Tiny filter, 100 items at 1% FP.  The vast majority of
        // non-inserted keys will be correctly rejected.
        let mut bf = BloomFilter::new(100, 0.01);
        for i in 0..50 {
            bf.insert(&rnd_bytes(i));
        }
        let mut false_positives = 0;
        let probes = 5_000;
        for i in 10_000..(10_000 + probes) {
            if bf.contains(&rnd_bytes(i)) {
                false_positives += 1;
            }
        }
        let rate = false_positives as f64 / probes as f64;
        // The theoretical FP rate is 1%.  Allow a 3x slack for sampling
        // noise on a small filter.
        assert!(
            rate < 0.03,
            "empirical FP rate {} exceeds 3% (probes={})",
            rate,
            probes
        );
    }

    #[cfg(feature = "bloom")]
    #[test]
    fn clear_resets_state() {
        let mut bf = BloomFilter::new(1_000, 0.01);
        let key: &[u8] = b"k";
        bf.insert(key);
        assert!(bf.contains(key));
        assert_eq!(bf.len(), 1);
        bf.clear();
        assert_eq!(bf.len(), 0);
        assert!(!bf.contains(key));
    }

    #[cfg(feature = "bloom")]
    #[test]
    fn default_filter_10k_at_1pct() {
        let bf = BloomFilter::with_defaults();
        // m ~ 95851 bits, rounded up to 2^17 = 131072.
        assert!(bf.m() >= 10_000);
        assert_eq!(bf.k(), 7); // ceil(-log2 0.01) = 7
        assert_eq!(bf.target_fp(), 0.01);
    }

    #[cfg(feature = "bloom")]
    #[test]
    fn no_false_negatives_under_stress() {
        // Insert N items, then verify *every* one of them is `contains ==
        // true`.  This is the structural guarantee of a Bloom filter and
        // should never fail.
        let mut bf = BloomFilter::new(2_000, 0.001);
        let n = 2_000;
        for i in 0..n {
            bf.insert(&rnd_bytes(i));
        }
        for i in 0..n {
            assert!(bf.contains(&rnd_bytes(i)), "false negative at i={}", i);
        }
        assert_eq!(bf.len(), n);
    }

    #[cfg(feature = "bloom")]
    #[test]
    fn empirical_fp_stays_near_target_under_load() {
        // 5000 items at 1% target FP.  Probe 20000 unrelated keys and
        // verify the empirical rate is within 3x of the target.
        let n = 5_000;
        let target = 0.01;
        let mut bf = BloomFilter::new(n, target);
        for i in 0..n {
            bf.insert(&rnd_bytes(i));
        }
        let probes = 20_000;
        let mut fps = 0;
        for i in (n + 1)..(n + 1 + probes) {
            if bf.contains(&rnd_bytes(i)) {
                fps += 1;
            }
        }
        let rate = fps as f64 / probes as f64;
        assert!(
            rate < target * 3.0,
            "empirical FP rate {} > 3x target {} (fps={}/{})",
            rate,
            target,
            fps,
            probes
        );
    }

    #[cfg(feature = "bloom")]
    #[test]
    fn popcount_monotonic() {
        let mut bf = BloomFilter::new(1_000, 0.01);
        let baseline = bf.popcount();
        for i in 0..100 {
            bf.insert(&rnd_bytes(i));
        }
        assert!(bf.popcount() > baseline);
    }
}
