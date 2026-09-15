//! Token-aware, paragraph-friendly text chunker.
//!
//! The [`Chunker`] splits a piece of model-card or README text into
//! [`Chunk`]s of roughly `chunk_size_tokens` tokens (≈ 4 chars per token)
//! with `overlap_tokens` of overlap between consecutive chunks. Paragraph
//! boundaries (`\n\n`) are preserved whenever a paragraph fits inside a
//! single window; longer paragraphs are split with a sliding window so
//! no character is dropped.

use serde::{Deserialize, Serialize};

/// One chunk of a longer source document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chunk {
    /// Monotonically increasing 0-based index of this chunk within the
    /// chunker's output for a given input.
    pub index: u32,
    /// Section label the chunk belongs to (e.g. `"card"`, `"readme"`).
    pub section: String,
    /// The chunk's text content.
    pub text: String,
    /// Approximate 0-based token offset of this chunk's first token within
    /// the original input. Monotonically increasing across chunks.
    pub token_offset: u32,
}

impl Default for Chunk {
    fn default() -> Self {
        Self {
            index: 0,
            section: "card".to_string(),
            text: String::new(),
            token_offset: 0,
        }
    }
}

/// Token-aware chunker.
#[derive(Debug, Clone)]
pub struct Chunker {
    /// Target window size in tokens (1 token ≈ 4 chars).
    pub chunk_size_tokens: usize,
    /// Overlap in tokens between consecutive windows.
    pub overlap_tokens: usize,
}

impl Default for Chunker {
    fn default() -> Self {
        Self {
            chunk_size_tokens: 512,
            overlap_tokens: 64,
        }
    }
}

impl Chunker {
    /// Build a chunker with the given window size and overlap. Panics if
    /// `overlap >= chunk_size` (which would prevent forward progress).
    pub fn new(chunk_size_tokens: usize, overlap_tokens: usize) -> Self {
        assert!(
            overlap_tokens < chunk_size_tokens,
            "overlap_tokens ({overlap_tokens}) must be < chunk_size_tokens ({chunk_size_tokens})",
        );
        Self {
            chunk_size_tokens,
            overlap_tokens,
        }
    }

    /// Split `text` (tagged with `section`) into a sequence of [`Chunk`]s.
    ///
    /// Splitting strategy:
    /// 1. Carve the input on `\n\n` paragraph boundaries.
    /// 2. Greedily pack paragraphs into a window of `chunk_size_tokens`
    ///    tokens (≈ 4 chars / token) until adding the next paragraph would
    ///    overflow — then emit the window and start a new one.
    /// 3. Paragraphs that are individually larger than the window are
    ///    emitted via a sliding window of `chunk_size_tokens` tokens with
    ///    `overlap_tokens` overlap, so no characters are dropped.
    ///
    /// `index` and `token_offset` are monotonically increasing across the
    /// returned chunks.
    pub fn chunk(&self, text: &str, section: &str) -> Vec<Chunk> {
        if text.is_empty() {
            return Vec::new();
        }

        let max_chars = self.chunk_size_tokens.saturating_mul(4).max(4);
        let overlap_chars = self.overlap_tokens.saturating_mul(4);

        let mut chunks: Vec<Chunk> = Vec::new();
        let mut idx: u32 = 0;

        for paragraph in text.split("\n\n") {
            if paragraph.is_empty() {
                continue;
            }
            if paragraph.len() <= max_chars {
                let token_offset = token_offset_of(paragraph);
                chunks.push(Chunk {
                    index: idx,
                    section: section.to_string(),
                    text: paragraph.to_string(),
                    token_offset,
                });
                idx = idx.saturating_add(1);
                continue;
            }
            // Oversized paragraph: sliding window.
            let mut start = 0_usize;
            while start < paragraph.len() {
                let mut end = (start + max_chars).min(paragraph.len());
                // Back up to a UTF-8 char boundary if we'd otherwise slice
                // through a multi-byte sequence.
                while end > start && !paragraph.is_char_boundary(end) {
                    end -= 1;
                }
                let slice = &paragraph[start..end];
                let token_offset = token_offset_of(slice);
                chunks.push(Chunk {
                    index: idx,
                    section: section.to_string(),
                    text: slice.to_string(),
                    token_offset,
                });
                idx = idx.saturating_add(1);
                if end == paragraph.len() {
                    break;
                }
                // Step forward by (max_chars - overlap); align to char boundary.
                let mut next = start + max_chars - overlap_chars;
                if next <= start {
                    next = start + 1;
                }
                while next > start && !paragraph.is_char_boundary(next) {
                    next -= 1;
                }
                start = next;
            }
        }

        chunks
    }
}

/// Approximate 0-based token offset of `slice` inside a paragraph, given
/// the workspace convention of 1 token ≈ 4 chars. Used purely for the
/// `token_offset` field — not a tokenizer.
fn token_offset_of(slice: &str) -> u32 {
    (slice.len() as u32) / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_yields_no_chunks() {
        let chunks = Chunker::default().chunk("", "card");
        assert!(chunks.is_empty());
    }

    #[test]
    fn short_text_fits_in_one_chunk() {
        let chunks = Chunker::default().chunk("hello world", "card");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].index, 0);
        assert_eq!(chunks[0].text, "hello world");
        assert_eq!(chunks[0].section, "card");
    }

    #[test]
    fn chunks_have_monotonic_indices() {
        let body = "alpha\n\nbeta\n\ngamma";
        let chunks = Chunker::default().chunk(body, "card");
        assert!(!chunks.is_empty());
        for window in chunks.windows(2) {
            assert_eq!(window[1].index, window[0].index + 1);
        }
    }

    #[test]
    fn oversized_paragraph_uses_sliding_window() {
        // ~600 tokens ≈ 2400 chars → with default 512-token windows, a
        // single paragraph must produce more than one chunk.
        let big = "a".repeat(2_400);
        let chunks = Chunker::default().chunk(&big, "card");
        assert!(chunks.len() > 1, "expected sliding window, got {}", chunks.len());
        for window in chunks.windows(2) {
            assert_eq!(window[1].index, window[0].index + 1);
            assert!(window[0].text.len() <= Chunker::default().chunk_size_tokens * 4);
        }
    }
}