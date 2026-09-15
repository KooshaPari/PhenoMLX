//! Variant collapse — collapse BM25 hits that share a quantized base into one
//! "model family" row.
//!
//! Most model registries publish the same weights at multiple quantizations
//! (`-Q4_K_M`, `-Q5_K_M`, `-GGUF`, `-GPTQ`, …). For browsing UX, you want to
//! show one row per *family* and let the user drill into the variants. The
//! [`CollapseRule`] defines what counts as a "variant suffix"; the
//! [`collapse_variants`] function then groups consecutive hits by their
//! shared *base id* (the id stripped of the first matching suffix).
//!
//! Provenance preservation: certain sub-id tokens (e.g. a finetune marker
//! `"-ft"` or `"-lora"`) signal that the row is semantically a *different*
//! model, even if the base id is identical. We expose a
//! [`CollapseRule::preserve_provenance`] list so callers can mark those
//! tokens as "do not collapse across".
//!
//! Suffix matching is **case-insensitive**: `a-Q4_K_M`, `a.q4_k_m`, and
//! `a-Q4-k_m` all collapse to `a`. The matching suffix list covers both
//! `"-Q4_K_M"` and `".q4_k_m"` style separators so callers don't have to
//! pick one.

use serde::{Deserialize, Serialize};

use crate::tantivy_store::IndexHit;

/// One collapsed group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollapsedHit {
    /// The shared base id (the longest of the group, after the first matching
    /// suffix has been stripped from the right).
    pub base_id: String,
    /// All ids in this group, in the order they were presented to
    /// [`collapse_variants`].
    pub variants: Vec<String>,
    /// The highest-scoring hit in the group.
    pub top_hit: IndexHit,
}

/// Rule describing how to identify variant rows to collapse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollapseRule {
    /// Quantization / format suffixes that, when stripped from the right of
    /// an id, yield the row's "base id".
    ///
    /// Strings are matched case-insensitively. Only the *first* matching
    /// suffix is stripped; see [`collapse_key`]. The default list covers the
    /// common GGUF / GPTQ / AWQ / ExL2 / safetensors-quant separators, with
    /// both `-` and `.` leading punctuation.
    #[serde(default = "default_collapse_quant_suffixes")]
    pub collapse_quant_suffixes: Vec<String>,

    /// Tokens that, when present anywhere in the id, mark a row as
    /// semantically distinct from any other row with the same base id.
    ///
    /// For example, with `preserve_provenance = ["-ft", "-lora"]`, the ids
    /// `["a-Q4_K_M", "a-ft-Q4_K_M"]` would *not* collapse into the same
    /// group. We do a simple substring match (case-sensitive) per token.
    #[serde(default)]
    pub preserve_provenance: Vec<String>,
}

fn default_collapse_quant_suffixes() -> Vec<String> {
    // Cover both the dotted lower-case form and the dashed upper-case form
    // that real model registries publish (HuggingFace uses both depending on
    // the uploader). `collapse_key` matches case-insensitively so either form
    // resolves to the same base id.
    vec![
        // dotted lowercase (the form the spec literally lists)
        ".q2_k".into(),
        ".q3_k".into(),
        ".q4_0".into(),
        ".q4_k_m".into(),
        ".q5_k_m".into(),
        ".q5_0".into(),
        ".q5_1".into(),
        ".q6_k".into(),
        ".q8_0".into(),
        ".gguf".into(),
        ".gptq".into(),
        ".awq".into(),
        ".exl2".into(),
        ".safetensors".into(),
        // dashed uppercase (the form real HF ids use, e.g. `Llama-3-8B-Q4_K_M`)
        "-q2_k".into(),
        "-q3_k".into(),
        "-q4_0".into(),
        "-q4_k_m".into(),
        "-q5_k_m".into(),
        "-q5_0".into(),
        "-q5_1".into(),
        "-q6_k".into(),
        "-q8_0".into(),
        "-gguf".into(),
        "-gptq".into(),
        "-awq".into(),
        "-exl2".into(),
        // bare upper-case (no separator)
        "q2_k".into(),
        "q3_k".into(),
        "q4_0".into(),
        "q4_k_m".into(),
        "q5_k_m".into(),
        "q6_k".into(),
        "q8_0".into(),
        "gguf".into(),
        "gptq".into(),
        "awq".into(),
        "exl2".into(),
    ]
}

impl Default for CollapseRule {
    fn default() -> Self {
        Self {
            collapse_quant_suffixes: default_collapse_quant_suffixes(),
            preserve_provenance: Vec::new(),
        }
    }
}

impl CollapseRule {
    /// Override the quant suffix list and return the new rule (builder-style).
    #[must_use]
    pub fn with_collapse_quant_suffixes<I, S>(mut self, suffixes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.collapse_quant_suffixes = suffixes.into_iter().map(Into::into).collect();
        self
    }

    /// Override the preserve-provenance list and return the new rule.
    #[must_use]
    pub fn with_preserve_provenance<I, S>(mut self, tokens: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.preserve_provenance = tokens.into_iter().map(Into::into).collect();
        self
    }
}

/// Strip the first occurrence of any `rule.collapse_quant_suffixes` from the
/// right of `id` and return what's left.
///
/// Matching is **case-insensitive** — `a-Q4_K_M` and `a.q4_k_m` both strip to
/// `a`. Only the *longest* suffix that matches wins (so `.q4_k_m` beats
/// `.q4_0` for the id `a.q4_k_m`), and only one suffix is removed per call
/// (no double-strip).
#[must_use]
pub fn collapse_key(id: &str, rule: &CollapseRule) -> String {
    let id_lower = id.to_ascii_lowercase();

    // Sort longest-first so e.g. `.q4_k_m` wins over `.q4_0` when both could
    // match a suffix like `Q4_K_M`. We sort by byte length (== char count for
    // ASCII suffixes, which is all we have here).
    let mut suffixes: Vec<&str> = rule.collapse_quant_suffixes.iter().map(String::as_str).collect();
    suffixes.sort_by_key(|s| std::cmp::Reverse(s.len()));

    for suffix in suffixes {
        let suffix_lower = suffix.to_ascii_lowercase();
        if suffix_lower.is_empty() {
            continue;
        }
        if let Some(stripped_len) = id_lower.len().checked_sub(suffix_lower.len()) {
            if id_lower[stripped_len..] == suffix_lower {
                // Match — return the original-case prefix.
                return id[..stripped_len].to_string();
            }
        }
    }
    id.to_string()
}

/// Returns true if `id` contains any of `rule.preserve_provenance` tokens.
fn has_provenance_marker(id: &str, rule: &CollapseRule) -> bool {
    rule.preserve_provenance
        .iter()
        .any(|tok| !tok.is_empty() && id.contains(tok.as_str()))
}

/// Group `hits` by their effective base id (after suffix stripping + provenance
/// check). Within each group, hits are presented in the same relative order
/// they were given.
///
/// Two hits land in *different* groups when:
///   - their effective base ids differ, **or**
///   - one (but not the other) has a preserve-provenance marker.
///
/// The output is sorted by the top-hit score (descending), ties broken by
/// base_id ascending for determinism.
#[must_use]
pub fn collapse_variants(hits: Vec<IndexHit>, rule: &CollapseRule) -> Vec<CollapsedHit> {
    use std::collections::BTreeMap;

    // We key each group by (effective_base_id, provenance_flag) so that two
    // hits with the same base but different provenance markers stay in
    // separate groups.
    let mut groups: BTreeMap<(String, bool), CollapsedHit> = BTreeMap::new();

    for hit in hits {
        let base = collapse_key(&hit.id, rule);
        let prov = has_provenance_marker(&hit.id, rule);
        let key = (base.clone(), prov);

        groups
            .entry(key)
            .and_modify(|g| {
                g.variants.push(hit.id.clone());
                // Keep the highest-scoring hit as `top_hit`. Since hits are
                // fed in score-descending order (as BM25 returns), the *first*
                // we see is the best — we only replace if a later one
                // somehow outscores it.
                if hit.score > g.top_hit.score {
                    g.top_hit = hit.clone();
                }
            })
            .or_insert_with(|| CollapsedHit {
                base_id: base,
                variants: vec![hit.id.clone()],
                top_hit: hit,
            });
    }

    let mut out: Vec<CollapsedHit> = groups.into_values().collect();
    out.sort_by(|a, b| {
        b.top_hit
            .score
            .partial_cmp(&a.top_hit.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.base_id.cmp(&b.base_id))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(id: &str, score: f32) -> IndexHit {
        IndexHit {
            id: id.to_string(),
            score,
        }
    }

    #[test]
    fn collapse_key_strips_dashed_uppercase_suffix() {
        let rule = CollapseRule::default();
        assert_eq!(collapse_key("a-Q4_K_M", &rule), "a");
        assert_eq!(collapse_key("a-Q5_K_M", &rule), "a");
        assert_eq!(collapse_key("a-Q8_0", &rule), "a");
        assert_eq!(collapse_key("plain", &rule), "plain");
    }

    #[test]
    fn collapse_key_strips_dotted_lowercase_suffix() {
        let rule = CollapseRule::default();
        assert_eq!(collapse_key("a.q4_k_m", &rule), "a");
        assert_eq!(collapse_key("a.gguf", &rule), "a");
    }

    #[test]
    fn collapse_key_strips_only_first_match() {
        // Construct a string that has two suffix candidates; only the first
        // (right-most) one should be removed.
        let rule = CollapseRule::default();
        let id = "a-Q4_K_M-Q8_0";
        assert_eq!(collapse_key(id, &rule), "a-Q4_K_M");
    }

    #[test]
    fn collapse_key_prefers_longest_suffix() {
        // `.q4_k_m` is longer than `.q4_0`; for `a.q4_k_m` the longer suffix
        // should win so the result is `a` not `a.q4`.
        let rule = CollapseRule::default();
        assert_eq!(collapse_key("a.q4_k_m", &rule), "a");
    }

    #[test]
    fn collapse_groups_variants_under_base() {
        let rule = CollapseRule::default();
        let hits = vec![
            hit("a-Q4_K_M", 1.0),
            hit("a-Q5_K_M", 0.9),
            hit("a-Q8_0", 0.8),
            hit("b", 0.5),
        ];
        let groups = collapse_variants(hits, &rule);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].base_id, "a");
        assert_eq!(groups[0].variants, vec!["a-Q4_K_M", "a-Q5_K_M", "a-Q8_0"]);
        assert_eq!(groups[1].base_id, "b");
        assert_eq!(groups[1].variants, vec!["b"]);
        assert!((groups[0].top_hit.score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn collapse_preserves_provenance_markers() {
        let rule = CollapseRule::default().with_preserve_provenance(vec!["-ft"]);
        let hits = vec![hit("a-Q4_K_M", 1.0), hit("a-ft-Q4_K_M", 0.5)];
        let groups = collapse_variants(hits, &rule);
        // "a-ft-Q4_K_M" has the provenance marker, so it stays in its own group.
        assert_eq!(groups.len(), 2);
        let ids: Vec<&str> = groups.iter().map(|g| g.base_id.as_str()).collect();
        // The provenance-marked one strips "Q4_K_M" → "a-ft"; the other → "a".
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"a-ft"));
    }
}