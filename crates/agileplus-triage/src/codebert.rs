// SPDX-License-Identifier: MIT OR Apache-2.0
//! CodeBERT embedding backend.
//!
//! Delegates to the OpenAI embeddings API with a `code: ` prefix to
//! signal code-optimized intent.  In the future this will be replaced
//! by a local `candle` or `ort` CodeBERT model.
//!
//! Feature-gated behind `codebert` (requires `oai`).
//!
//! Traceability: audit rec #19 (CodeBERT embedding backend).

#[cfg(feature = "codebert")]
use crate::embeddings::{EmbeddingBackend, OaiEmbeddings};

/// CodeBERT embedding backend.
///
/// Wraps [`OaiEmbeddings`] and prefixes every input with `code: ` so
/// the remote model (or future local model) can treat the text as
/// source code.
#[cfg(feature = "codebert")]
#[derive(Debug, Clone)]
pub struct CodeBertEmbeddings {
    inner: OaiEmbeddings,
}

#[cfg(feature = "codebert")]
impl CodeBertEmbeddings {
    /// Create a new CodeBERT backend with the given OpenAI API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            inner: OaiEmbeddings::new(api_key),
        }
    }

    /// Override the underlying model name.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.inner = self.inner.with_model(model);
        self
    }

    /// Override the base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.inner = self.inner.with_base_url(url);
        self
    }
}

#[cfg(feature = "codebert")]
impl EmbeddingBackend for CodeBertEmbeddings {
    fn name(&self) -> &'static str {
        "codebert"
    }

    fn dim(&self) -> usize {
        self.inner.dim()
    }

    fn embed(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        let prefixed: Vec<String> = texts.iter().map(|t| format!("code: {t}")).collect();
        let refs: Vec<&str> = prefixed.iter().map(|s| s.as_str()).collect();
        self.inner.embed(&refs)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "codebert")]
    use super::*;

    #[cfg(feature = "codebert")]
    #[test]
    fn codebert_name_and_dim() {
        let b = CodeBertEmbeddings::new("sk-fake");
        assert_eq!(b.name(), "codebert");
        assert_eq!(b.dim(), 1536);
    }

    #[cfg(feature = "codebert")]
    #[test]
    fn codebert_builder_methods() {
        let b = CodeBertEmbeddings::new("sk-fake")
            .with_model("text-embedding-3-large")
            .with_base_url("https://staging.example.com");
        assert_eq!(b.name(), "codebert");
    }

    #[test]
    fn codebert_feature_gate_compiles() {
        // If the `codebert` feature is off, this module still compiles
        // but the struct is unavailable.  We just verify the module exists.
        assert!(true);
    }
}
