// SPDX-License-Identifier: MIT OR Apache-2.0
//! Embedding backends for semantic similarity.  Pluggable over a small
//! trait so the same hybrid dedup pipeline can target OpenAI,
//! Voyage AI, or a local deterministic mock depending on which
//! features are enabled at build time.
//!
//! # Trait
//!
//! ```ignore
//! pub trait EmbeddingBackend {
//!     fn embed(&self, texts: &[&str]) -> Vec<Vec<f32>>;
//!     fn name(&self) -> &'static str;
//! }
//! ```
//!
//! # Feature gates
//!
//! | Feature  | Pulls in | What you get |
//! |----------|----------|--------------|
//! | `local`  | —        | `LocalMockEmbeddings` (deterministic, no network) |
//! | `oai`    | `ureq`   | `OaiEmbeddings` → api.openai.com |
//! | `voyage` | `ureq`   | `VoyageEmbeddings` → api.voyageai.com |
//!
//! The default features are `["local", "bloom"]`, so a vanilla
//! `cargo build` (or `cargo test`) works offline with the local mock.
//!
//! # Audit
//!
//! Implements recommendation #3 from `AUDIT_BLOC_VS_2026_SOTA.md`:
//! pluggable embedding backends with a `local` no-network default.

/// A backend that maps a batch of input strings to fixed-dimension
/// embedding vectors.  Implementations should be deterministic for the
/// same input (the local mock is; the remote ones are deterministic
/// modulo model version).
pub trait EmbeddingBackend {
    /// Compute one embedding per input text.  The output order matches
    /// the input order.  Each vector has `dim()` floats.
    fn embed(&self, texts: &[&str]) -> Vec<Vec<f32>>;
    /// Stable backend name (e.g. `"oai"`, `"voyage"`, `"local-mock"`).
    fn name(&self) -> &'static str;
    /// Embedding dimension for this backend.
    fn dim(&self) -> usize;
}

/// Cosine similarity in `[-1.0, 1.0]`.  Zero vectors return `0.0`.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0f64, 0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(b.iter()) {
        let (x, y) = (*x as f64, *y as f64);
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        (dot / denom) as f32
    }
}

// ---------------------------------------------------------------------------
// Local mock — deterministic, no network, no extra deps.
// ---------------------------------------------------------------------------

/// Deterministic, network-free embedding backend.  Useful for tests, CI,
/// and offline development.  Each input text is hashed token-by-token
/// into a fixed `dim` vector and L2-normalized.
#[derive(Debug, Clone)]
pub struct LocalMockEmbeddings {
    dim: usize,
}

impl Default for LocalMockEmbeddings {
    fn default() -> Self {
        Self { dim: 384 }
    }
}

impl LocalMockEmbeddings {
    /// Construct a mock backend with the given embedding dimension.
    /// `384` matches `nomic-embed-text-v1.5`, a common local default.
    pub fn new(dim: usize) -> Self {
        assert!(dim > 0, "LocalMockEmbeddings dim must be > 0");
        Self { dim }
    }
}

impl EmbeddingBackend for LocalMockEmbeddings {
    fn name(&self) -> &'static str {
        "local-mock"
    }
    fn dim(&self) -> usize {
        self.dim
    }
    fn embed(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        texts.iter().map(|t| self.embed_one(t)).collect()
    }
}

impl LocalMockEmbeddings {
    fn embed_one(&self, text: &str) -> Vec<f32> {
        // Tokenize with the same policy as the dedup module (lowercase,
        // alnum-only, len>=2).  We hash each token with FNV-1a 64-bit
        // and fold the hash into `dim` buckets using the "feature
        // hashing" trick (Weinberger et al. 2009).  Sign of the
        // contribution is the parity of the *other* half of the hash.
        let mut v = vec![0.0f32; self.dim];
        for tok in tokenize(text) {
            let (bucket, sign) = feature_hash(&tok, self.dim);
            v[bucket] += sign;
        }
        // L2 normalize so cosine reduces to dot product.
        let norm = v.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x = (*x as f64 / norm) as f32;
            }
        }
        v
    }
}

fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 2)
        .map(String::from)
        .collect()
}

/// 64-bit FNV-1a.  Returns the bucket index in `[0, dim)` and a sign in
/// `{-1.0, +1.0}` (the sign of the upper bit, mapped to ±1).
fn feature_hash(token: &str, dim: usize) -> (usize, f32) {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x100_0000_01b3;
    let mut h = FNV_OFFSET;
    for &b in token.as_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    let bucket = (h as usize) % dim;
    let sign = if (h >> 63) & 1 == 0 { 1.0 } else { -1.0 };
    (bucket, sign)
}

// ---------------------------------------------------------------------------
// OpenAI — `oai` feature
// ---------------------------------------------------------------------------

/// OpenAI embeddings backend.  Posts to
/// `https://api.openai.com/v1/embeddings` with model
/// `text-embedding-3-small`.  Requires the `oai` feature (which pulls in
/// `ureq`).
#[cfg(feature = "oai")]
#[derive(Debug, Clone)]
pub struct OaiEmbeddings {
    api_key: String,
    model: String,
    base_url: String,
}

#[cfg(feature = "oai")]
impl OaiEmbeddings {
    /// Construct an OpenAI client with the default model
    /// (`text-embedding-3-small`).
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "text-embedding-3-small".to_string(),
            base_url: "https://api.openai.com".to_string(),
        }
    }
    /// Override the model name (e.g. `text-embedding-3-large`).
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
    /// Override the base URL (for staging or local proxies).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

#[cfg(feature = "oai")]
impl EmbeddingBackend for OaiEmbeddings {
    fn name(&self) -> &'static str {
        "oai"
    }
    fn dim(&self) -> usize {
        // text-embedding-3-small returns 1536-d; -large returns 3072-d.
        // The real value comes back in the API response; we just
        // advertise the small-model default.
        1536
    }
    fn embed(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        use serde::{Deserialize, Serialize};
        if texts.is_empty() {
            return Vec::new();
        }
        #[derive(Serialize)]
        struct Req<'a> {
            input: Vec<&'a str>,
            model: &'a str,
        }
        #[derive(Deserialize)]
        struct Resp {
            data: Vec<RespItem>,
        }
        #[derive(Deserialize)]
        struct RespItem {
            embedding: Vec<f32>,
        }
        let req = Req {
            input: texts.to_vec(),
            model: &self.model,
        };
        let url = format!("{}/v1/embeddings", self.base_url);
        let resp: Resp = ureq::post(&url)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_json(serde_json::to_value(&req).expect("serialize req"))
            .expect("OaiEmbeddings: HTTP request failed")
            .into_json()
            .expect("OaiEmbeddings: response parse failed");
        resp.data.into_iter().map(|d| d.embedding).collect()
    }
}

// ---------------------------------------------------------------------------
// Voyage AI — `voyage` feature
// ---------------------------------------------------------------------------

/// Voyage AI embeddings backend.  Posts to
/// `https://api.voyageai.com/v1/embeddings` with model `voyage-3`.
#[cfg(feature = "voyage")]
#[derive(Debug, Clone)]
pub struct VoyageEmbeddings {
    api_key: String,
    model: String,
    base_url: String,
}

#[cfg(feature = "voyage")]
impl VoyageEmbeddings {
    /// Construct a Voyage client with the default model (`voyage-3`).
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "voyage-3".to_string(),
            base_url: "https://api.voyageai.com".to_string(),
        }
    }
    /// Override the model name.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
    /// Override the base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

#[cfg(feature = "voyage")]
impl EmbeddingBackend for VoyageEmbeddings {
    fn name(&self) -> &'static str {
        "voyage"
    }
    fn dim(&self) -> usize {
        1024
    }
    fn embed(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        use serde::{Deserialize, Serialize};
        if texts.is_empty() {
            return Vec::new();
        }
        #[derive(Serialize)]
        struct Req<'a> {
            input: Vec<&'a str>,
            model: &'a str,
        }
        #[derive(Deserialize)]
        struct Resp {
            data: Vec<RespItem>,
        }
        #[derive(Deserialize)]
        struct RespItem {
            embedding: Vec<f32>,
        }
        let req = Req {
            input: texts.to_vec(),
            model: &self.model,
        };
        let url = format!("{}/v1/embeddings", self.base_url);
        let resp: Resp = ureq::post(&url)
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_json(serde_json::to_value(&req).expect("serialize req"))
            .expect("VoyageEmbeddings: HTTP request failed")
            .into_json()
            .expect("VoyageEmbeddings: response parse failed");
        resp.data.into_iter().map(|d| d.embedding).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn cosine_of_identical_vectors_is_one() {
        let v = vec![1.0, 2.0, 3.0, 4.0];
        assert!(approx(cosine(&v, &v), 1.0, 1e-6));
    }

    #[test]
    fn cosine_of_orthogonal_vectors_is_zero() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(approx(cosine(&a, &b), 0.0, 1e-6));
    }

    #[test]
    fn cosine_of_opposite_vectors_is_minus_one() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        assert!(approx(cosine(&a, &b), -1.0, 1e-6));
    }

    #[test]
    fn cosine_of_zero_vector_is_zero() {
        let z = vec![0.0, 0.0, 0.0];
        let v = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine(&z, &v), 0.0);
        assert_eq!(cosine(&v, &z), 0.0);
    }

    #[test]
    fn cosine_handles_mismatched_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine(&a, &b), 0.0);
    }

    #[test]
    fn local_mock_dim_is_384_by_default() {
        let b = LocalMockEmbeddings::default();
        assert_eq!(b.dim(), 384);
        assert_eq!(b.name(), "local-mock");
    }

    #[test]
    fn local_mock_embeds_to_correct_shape() {
        let b = LocalMockEmbeddings::new(64);
        let inputs = vec!["hello world", "foo bar", ""];
        let embs = b.embed(&inputs);
        assert_eq!(embs.len(), 3);
        for (i, e) in embs.iter().enumerate() {
            assert_eq!(e.len(), 64, "input {i} has wrong dim");
        }
    }

    #[test]
    fn local_mock_vectors_are_l2_normalized() {
        let b = LocalMockEmbeddings::new(128);
        let embs = b.embed(&["the quick brown fox", "lorem ipsum dolor sit amet"]);
        for e in &embs {
            let norm: f64 = e.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
            // Empty input -> zero vector; otherwise the norm should be ~1.
            let sum: f64 = e.iter().map(|x| (*x as f64).abs()).sum();
            if sum > 0.0 {
                assert!(
                    approx(norm as f32, 1.0, 1e-3),
                    "vector not L2-normalized: norm={norm}"
                );
            }
        }
    }

    #[test]
    fn local_mock_is_deterministic() {
        let b = LocalMockEmbeddings::new(128);
        let a = b.embed(&["hello world"]);
        let c = b.embed(&["hello world"]);
        assert_eq!(a, c, "LocalMockEmbeddings must be deterministic");
    }

    #[test]
    fn local_mock_similar_texts_have_higher_cosine_than_disjoint() {
        let b = LocalMockEmbeddings::new(256);
        let embs = b.embed(&[
            "implement authentication flow using oauth2",
            "implement authentication flow with oauth2 (slight reword)",
            "completely unrelated content about cooking recipes",
        ]);
        let sim_close = cosine(&embs[0], &embs[1]);
        let sim_far = cosine(&embs[0], &embs[2]);
        assert!(
            sim_close > sim_far,
            "similar texts should have higher cosine: close={sim_close} far={sim_far}"
        );
    }

    #[test]
    fn local_mock_handles_empty_input_batch() {
        let b = LocalMockEmbeddings::new(32);
        let embs = b.embed(&[]);
        assert!(embs.is_empty());
    }

    #[test]
    fn local_mock_handles_empty_string_input() {
        let b = LocalMockEmbeddings::new(32);
        let embs = b.embed(&[""]);
        assert_eq!(embs.len(), 1);
        assert_eq!(embs[0].len(), 32);
        // Empty input tokenizes to nothing -> zero vector.
        let sum: f32 = embs[0].iter().map(|x| x.abs()).sum();
        assert_eq!(sum, 0.0);
    }

    #[test]
    fn local_mock_dim_customization() {
        for d in [16usize, 64, 128, 768, 1536] {
            let b = LocalMockEmbeddings::new(d);
            assert_eq!(b.dim(), d);
            let embs = b.embed(&["some text"]);
            assert_eq!(embs[0].len(), d);
        }
    }

    // -------- feature-gated construction tests --------

    #[cfg(feature = "oai")]
    #[test]
    fn oai_construction_with_api_key() {
        let b = OaiEmbeddings::new("sk-fake");
        assert_eq!(b.name(), "oai");
        assert_eq!(b.dim(), 1536);
        // The model is `text-embedding-3-small` by default.
        let b2 = OaiEmbeddings::new("k").with_model("text-embedding-3-large");
        // We don't expose `model` for inspection; we just confirm the
        // builder runs and `dim` is unchanged (the real dim is given by
        // the API response, not the client).
        assert_eq!(b2.name(), "oai");
    }

    #[cfg(feature = "voyage")]
    #[test]
    fn voyage_construction_with_api_key() {
        let b = VoyageEmbeddings::new("pa-fake");
        assert_eq!(b.name(), "voyage");
        assert_eq!(b.dim(), 1024);
        let b2 = VoyageEmbeddings::new("k").with_model("voyage-code-3");
        assert_eq!(b2.name(), "voyage");
    }
}
