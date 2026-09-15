//! Quantization tagger.
//!
//! Scans the list of `tree_entries` (file paths) on a
//! [`RawModel`](hwledger_search_core::RawModel) and classifies them by suffix / filename token so downstream indexers can
//! facet over `gguf / gptq / awq / exl2 / safetensors` without re-doing the
//! string match.
//!
//! The set of recognised GGUF quant suffixes is the conventional llama.cpp
//! `Q*_K_M / Q*_K_S / Q*_0 / Q*_1` family. We store the canonical short
//! form (lowercase, no separators) so the surface area is `q4_k_m`,
//! `q5_k_m`, `q8_0`, `q2_k`, etc.
//!
//! The module never panics; if no `tree_entries` are present we return the
//! default ("nothing quant, nothing present") shape.

use crate::tager_context::TaggerContext;

/// Quantization facet for a single model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuantTags {
    /// Canonical quant flavors discovered across the tree entries (e.g.
    /// `"q4_k_m"`, `"q5_k_m"`, `"q8_0"`, `"q2_k"`). Stored lowercase, sorted
    /// later by the orchestrator.
    pub quants: Vec<String>,
    /// At least one `.gguf` file is present.
    pub gguf_present: bool,
    /// A filename contains the literal `gptq` token.
    pub gptq_present: bool,
    /// A filename contains the literal `awq` token.
    pub awq_present: bool,
    /// A filename contains the literal `exl2` token.
    pub exl2_present: bool,
    /// At least one `.safetensors` file is present.
    pub safetensors_present: bool,
}

/// Classify a [`TaggerContext`]'s file tree by quantization format.
pub fn tag(ctx: &TaggerContext) -> QuantTags {
    let mut out = QuantTags::default();

    let Some(raw) = ctx.raw() else {
        return out;
    };

    for entry in &raw.tree_entries {
        let lower = entry.to_ascii_lowercase();
        let fname = lower.rsplit('/').next().unwrap_or(&lower);

        if fname.ends_with(".gguf") {
            out.gguf_present = true;
            if let Some(parsed) = parse_gguf_quant(fname) {
                if !out.quants.iter().any(|q| q == &parsed) {
                    out.quants.push(parsed);
                }
            }
        }

        if fname.ends_with(".safetensors") || fname.contains(".safetensors.") {
            out.safetensors_present = true;
        }

        // Tokenize on non-alphanumerics so "gptq-awq-v2" splits correctly.
        if fname.split(|c: char| !c.is_ascii_alphanumeric()).any(|tok| tok == "gptq") {
            out.gptq_present = true;
        }
        if fname.split(|c: char| !c.is_ascii_alphanumeric()).any(|tok| tok == "awq") {
            out.awq_present = true;
        }
        if fname.split(|c: char| !c.is_ascii_alphanumeric()).any(|tok| tok == "exl2") {
            out.exl2_present = true;
        }
    }

    out
}

/// Try to extract a GGUF quant suffix from a filename. Returns the canonical
/// lowercase token (e.g. `q4_k_m`) if a recognised pattern is found, else
/// `None`. We support the full `Q*_K_* / Q*_0 / Q*_1 / Q*_S / Q*_M` family.
fn parse_gguf_quant(fname: &str) -> Option<String> {
    // Strip the .gguf suffix first so we don't accidentally match a quant
    // token after the extension.
    let stem = fname.strip_suffix(".gguf").unwrap_or(fname);

    // Find the rightmost `Q<digits>[_<letter>[_<letter>]]` token. We
    // anchor on an uppercase-or-lowercase `q` to match both forms.
    let bytes = stem.as_bytes();
    let mut best: Option<(usize, &str)> = None;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'q' || c == b'Q' {
            // Look ahead: `q` followed by at least one digit and an optional
            // `_K_M` / `_K_S` / `_0` / `_1` style tail.
            let tail = &stem[i..];
            if let Some(suffix) = match_q_suffix(tail) {
                let end = i + suffix.len();
                best = match best {
                    Some((prev_end, _)) if prev_end > end => best,
                    _ => Some((end, suffix)),
                };
            }
        }
        i += 1;
    }

    best.map(|(_, s)| s.to_string())
}

/// Match a single `Q*_K_M / Q*_K_S / Q*_0 / Q*_1 / Q*_S / Q*_M` token at the
/// start of `s`. Returns the canonical lowercase form when matched.
fn match_q_suffix(s: &str) -> Option<&'static str> {
    // Must be q1q2q3q4… style. Walk through char-by-char.
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() || (chars[0] != 'q' && chars[0] != 'Q') {
        return None;
    }

    // Collect [digits].
    let mut end = 1;
    while end < chars.len() && chars[end].is_ascii_digit() {
        end += 1;
    }
    if end == 1 {
        return None;
    }
    let digits = &chars[1..end];
    let bits: u32 = digits.iter().filter_map(|c| c.to_digit(10)).sum();

    // Now classify by the trailing tail.
    let tail: String = chars[end..].iter().take(8).collect();
    let tail_lc = tail.to_ascii_lowercase();

    // Q2_K / Q4_K_M / Q5_K_S / Q6_K
    if let Some(after_k) = tail_lc.strip_prefix("_k") {
        if let Some(kind) = after_k.strip_prefix('_') {
            if kind.starts_with('m') {
                return Some(canonical_q(bits, "k_m"));
            }
            if kind.starts_with('s') {
                return Some(canonical_q(bits, "k_s"));
            }
        }
        // Bare Q?_K
        return Some(canonical_q(bits, "k"));
    }
    // Q8_0 / Q4_0 / Q4_1 / Q5_1
    if let Some(rest) = tail_lc.strip_prefix('_') {
        if rest.len() == 1 && rest.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()) {
            return Some(canonical_q(bits, rest));
        }
    }
    // Q4_S / Q5_M (no underscore-K) — llama.cpp legacy.
    if tail_lc.len() == 1 {
        let c = tail_lc.chars().next().unwrap();
        if c == 's' || c == 'm' {
            return Some(canonical_q(bits, &c.to_string()));
        }
    }

    None
}

/// Build the canonical lowercase token for a given quant bit width + suffix.
fn canonical_q(bits: u32, suffix: &str) -> &'static str {
    match (bits, suffix) {
        (2, "k") => "q2_k",
        (2, "k_m") => "q2_k_m",
        (2, "k_s") => "q2_k_s",
        (3, "k") => "q3_k",
        (3, "k_m") => "q3_k_m",
        (3, "k_s") => "q3_k_s",
        (4, "k") => "q4_k",
        (4, "k_m") => "q4_k_m",
        (4, "k_s") => "q4_k_s",
        (4, "0") => "q4_0",
        (4, "1") => "q4_1",
        (4, "s") => "q4_s",
        (4, "m") => "q4_m",
        (5, "k") => "q5_k",
        (5, "k_m") => "q5_k_m",
        (5, "k_s") => "q5_k_s",
        (5, "1") => "q5_1",
        (5, "m") => "q5_m",
        (6, "k") => "q6_k",
        (6, "0") => "q6_0",
        (7, "0") => "q7_0",
        (8, "0") => "q8_0",
        _ => "q_unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gguf_quant_q4_k_m() {
        assert_eq!(parse_gguf_quant("model.Q4_K_M.gguf").as_deref(), Some("q4_k_m"));
    }

    #[test]
    fn parse_gguf_quant_q5_k_m() {
        assert_eq!(parse_gguf_quant("llama-3-8b.Q5_K_M.gguf").as_deref(), Some("q5_k_m"));
    }

    #[test]
    fn parse_gguf_quant_q8_0() {
        assert_eq!(parse_gguf_quant("model.Q8_0.gguf").as_deref(), Some("q8_0"));
    }

    #[test]
    fn parse_gguf_quant_q2_k() {
        assert_eq!(parse_gguf_quant("model.Q2_K.gguf").as_deref(), Some("q2_k"));
    }

    #[test]
    fn parse_gguf_quant_unknown_bits() {
        // Bits outside the canonical 2..=8 range fall through to `q_unknown`.
        assert_eq!(parse_gguf_quant("model.Q42_K_M.gguf").as_deref(), Some("q_unknown"));
    }

    #[test]
    fn parse_gguf_quant_unknown_suffix() {
        // A suffix shape we don't recognize returns `None` rather than a
        // mismatch-tag, so the orchestrator can tell the difference
        // between "definitely a quant, but not one we know" and "not a
        // quant at all".
        assert_eq!(parse_gguf_quant("model.Q42_ZZ.gguf"), None);
    }

    #[test]
    fn parse_gguf_quant_no_quant() {
        assert_eq!(parse_gguf_quant("config.json"), None);
    }
}
