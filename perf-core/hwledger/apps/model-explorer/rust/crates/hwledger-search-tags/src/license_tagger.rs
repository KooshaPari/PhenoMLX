//! License tagger.
//!
//! Extracts the license string from either the HF `config.json` or the
//! model card, and flags a coarse `restrictive` boolean for downstream
//! filtering (e.g. "block this for commercial users").
//!
//! The restrictive heuristic is intentionally conservative: a license is
//! flagged restrictive if it contains any of the well-known non-commercial
//! tokens (`cc-by-nc`, `rail`, `gemma`, `llama3`, `research-only`,
//! `non-commercial`). License families we can't classify stay `false`.

use crate::tager_context::TaggerContext;

/// License classification for a single model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LicenseTags {
    /// Canonical license string (lowercased, trimmed), if any.
    pub license: Option<String>,
    /// `true` if the license is non-commercial / has use restrictions.
    pub restrictive: bool,
    /// Family bucket — e.g. `"apache"`, `"mit"`, `"cc-by"`, `"proprietary"`,
    /// `"other"`. We don't try to be exhaustive here; downstream consumers
    /// are expected to facet on the raw `license` string anyway.
    pub family: Option<String>,
}

/// Heuristic license extraction.
pub fn tag(ctx: &TaggerContext) -> LicenseTags {
    let raw = ctx.raw();

    // 1. Try config_json['license'] first.
    let from_cfg = raw
        .and_then(|r| r.config_json.as_ref())
        .and_then(|c| c.get("license"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // 2. Fall back to scanning the model card for a `License:` line.
    let from_card = if from_cfg.is_none() {
        ctx.effective_card_text()
            .and_then(parse_license_from_card)
    } else {
        None
    };

    let license = from_cfg.or(from_card);
    let lower = license.as_deref().map(|s| s.to_ascii_lowercase());
    let restrictive = lower
        .as_deref()
        .is_some_and(is_restrictive);
    let family = lower.as_deref().and_then(classify_family);

    LicenseTags {
        license,
        restrictive,
        family,
    }
}

/// Pull the first `License:` line from a model card. Returns the trimmed
/// remainder after the colon, lowercase-normalised.
fn parse_license_from_card(card: &str) -> Option<String> {
    // Look for a line whose left-hand side (after trimming) starts with
    // "license" (case-insensitive) followed by a colon.
    for line in card.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed
            .strip_prefix("License")
            .or_else(|| trimmed.strip_prefix("license"))
        {
            let rest = rest.trim_start();
            if let Some(after_colon) = rest.strip_prefix(':') {
                let val = after_colon.trim();
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

/// `true` if the license string contains a non-commercial token.
fn is_restrictive(lower: &str) -> bool {
    const TOKENS: &[&str] = &[
        "cc-by-nc",
        "cc by-nc",
        "non-commercial",
        "non commercial",
        "research-only",
        "research only",
        "rail",
        "gemma",
        "llama3",
        "llama-3",
    ];
    TOKENS.iter().any(|t| lower.contains(t))
}

/// Map a license string to a coarse family bucket.
fn classify_family(lower: &str) -> Option<String> {
    if lower.contains("apache") {
        Some("apache".to_string())
    } else if lower.contains("mit") {
        Some("mit".to_string())
    } else if lower.starts_with("cc-by") || lower.starts_with("cc by") {
        Some("cc-by".to_string())
    } else if lower.contains("gemma") || lower.contains("llama") || lower.contains("mistral") {
        Some("proprietary".to_string())
    } else {
        Some("other".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restrictive_flags_nc() {
        // `is_restrictive` is called with already-lowercased input from
        // `tag()`. The unit tests mirror that contract.
        assert!(is_restrictive("cc-by-nc-4.0"));
        assert!(is_restrictive("llama3 community license"));
        assert!(is_restrictive("gemma terms of use"));
        assert!(is_restrictive("rail license"));
        assert!(is_restrictive("custom non-commercial license"));
    }

    #[test]
    fn permissive_does_not_flag() {
        assert!(!is_restrictive("apache-2.0"));
        assert!(!is_restrictive("mit"));
        assert!(!is_restrictive("cc-by-4.0"));
        assert!(!is_restrictive("bsd-3-clause"));
    }

    #[test]
    fn license_from_card() {
        let card = "# Model Card\n\nLicense: apache-2.0\n\nSome other text.";
        assert_eq!(parse_license_from_card(card).as_deref(), Some("apache-2.0"));
    }

    #[test]
    fn license_from_card_missing() {
        let card = "# Model Card\n\nNo license line here.";
        assert_eq!(parse_license_from_card(card), None);
    }
}
