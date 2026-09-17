//! Normalizer: a `WorkPackage` is already the normalized shape, so this module
//! provides cross-format helpers: slug, hash, merge.

use crate::WorkPackage;

/// Produce a stable slug from a work package ID and source anchor.
pub fn slug(pkg: &WorkPackage) -> String {
    let raw = format!("{}-{}", pkg.source_format, pkg.source_anchor);
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Stable 16-char hash for dedup across formats (FNV-1a 64-bit, hex).
pub fn stable_hash(pkg: &WorkPackage) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in slug(pkg).as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

/// Merge two packages with the same hash: prefer the one with more acceptance
/// criteria, but keep the longer description.
pub fn merge(a: WorkPackage, b: WorkPackage) -> WorkPackage {
    if a.acceptance.len() >= b.acceptance.len() {
        let mut out = a;
        if b.description.len() > out.description.len() {
            out.description = b.description;
        }
        out
    } else {
        let mut out = b;
        if a.description.len() > out.description.len() {
            out.description = a.description;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AcceptanceCriterion, WorkPackage};

    fn pkg(anchor: &str) -> WorkPackage {
        WorkPackage {
            id: format!("x-{}", anchor),
            title: "T".into(),
            description: "D".into(),
            acceptance: vec![AcceptanceCriterion { text: "a".into(), done: false }],
            source_format: "x".into(),
            source_anchor: anchor.into(),
        }
    }

    #[test]
    fn slug_strips_separators() {
        let p = pkg("ABC-1");
        assert_eq!(slug(&p), "x-abc-1");
    }

    #[test]
    fn stable_hash_is_deterministic() {
        let p = pkg("X-1");
        assert_eq!(stable_hash(&p), stable_hash(&p));
        assert_eq!(stable_hash(&p).len(), 16);
    }

    #[test]
    fn merge_picks_more_acceptance() {
        let a = WorkPackage { acceptance: vec![], ..pkg("X") };
        let mut b = pkg("X");
        b.acceptance.push(AcceptanceCriterion { text: "b".into(), done: false });
        let m = merge(a, b);
        assert_eq!(m.acceptance.len(), 2);
    }
}
