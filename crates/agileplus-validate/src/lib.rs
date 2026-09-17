// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shared validation rules for AgilePlus domain and CLI inputs.

use std::path::{Component, Path};

const BRANCH_PREFIXES: [&str; 6] = ["feat/", "fix/", "chore/", "ci/", "docs/", "refactor/"];

/// Require a non-empty human-readable name after trimming surrounding whitespace.
pub fn name_required(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("name must not be empty".to_owned());
    }

    Ok(())
}

/// Require a lowercase kebab-case slug made of ASCII letters, digits, and hyphens.
pub fn slug_format(slug: &str) -> Result<(), String> {
    if slug.is_empty() {
        return Err("slug must not be empty".to_owned());
    }

    if slug.starts_with('-') || slug.ends_with('-') {
        return Err("slug must not start or end with '-'".to_owned());
    }

    if slug.contains("--") {
        return Err("slug must not contain consecutive hyphens".to_owned());
    }

    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("slug must contain only lowercase letters, digits, and hyphens".to_owned());
    }

    Ok(())
}

/// Require an x.y.z semantic version where each numeric segment parses as u64.
pub fn semver_format(version: &str) -> Result<(), String> {
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3 {
        return Err("version must contain exactly three dot-separated numeric parts".to_owned());
    }

    if parts
        .iter()
        .any(|part| part.is_empty() || part.parse::<u64>().is_err())
    {
        return Err("version parts must be unsigned integers".to_owned());
    }

    Ok(())
}

/// Require a branch name that uses a supported AgilePlus work prefix.
pub fn branch_naming_convention(branch: &str) -> Result<(), String> {
    let Some(prefix) = BRANCH_PREFIXES
        .iter()
        .find(|prefix| branch.starts_with(**prefix))
    else {
        return Err(
            "branch must start with feat/, fix/, chore/, ci/, docs/, or refactor/".to_owned(),
        );
    };

    let suffix = &branch[prefix.len()..];
    if suffix.is_empty() {
        return Err("branch suffix must not be empty".to_owned());
    }

    if suffix.starts_with('/') || suffix.ends_with('/') || suffix.contains("//") {
        return Err("branch suffix must not start, end, or repeat '/'".to_owned());
    }

    for segment in suffix.split('/') {
        slug_format(segment)
            .map_err(|_| "branch suffix segments must be lowercase kebab-case".to_owned())?;
    }

    Ok(())
}

/// Allow only artifacts under kitty-specs/ or docs/ and reject path traversal.
pub fn path_allowlist(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    let mut components = path.components();
    let Some(first) = components.next() else {
        return Err("path must not be empty".to_owned());
    };

    let Component::Normal(root) = first else {
        return Err("path must be relative to kitty-specs/ or docs/".to_owned());
    };

    if root != "kitty-specs" && root != "docs" {
        return Err("path must stay within kitty-specs/ or docs/".to_owned());
    }

    for component in components {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            Component::ParentDir => return Err("path traversal is not allowed".to_owned()),
            Component::RootDir | Component::Prefix(_) => {
                return Err("path must be relative to kitty-specs/ or docs/".to_owned());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn slug_regex() -> &'static str {
        "[a-z0-9]+(-[a-z0-9]+)*"
    }

    fn branch_suffix_regex() -> &'static str {
        "[a-z0-9]+([/-][a-z0-9]+)*"
    }

    fn path_suffix_regex() -> &'static str {
        "[a-z0-9_./-]{0,32}"
    }

    fn semver_part_regex() -> &'static str {
        "[0-9]{1,6}"
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn proptest_name_required_invariants(name in "[[:alnum:] _-]{1,32}") {
            prop_assume!(!name.trim().is_empty());
            prop_assert!(name_required(&name).is_ok());
        }

        #[test]
        fn proptest_slug_invariants(slug in slug_regex()) {
            prop_assert!(slug_format(&slug).is_ok());
            prop_assert!(!slug.starts_with('-'));
            prop_assert!(!slug.ends_with('-'));
            prop_assert!(!slug.contains("--"));
        }

        #[test]
        fn proptest_semver_invariants(
            major in semver_part_regex(),
            minor in semver_part_regex(),
            patch in semver_part_regex()
        ) {
            let version = format!("{major}.{minor}.{patch}");
            prop_assert!(semver_format(&version).is_ok());
        }

        #[test]
        fn proptest_branch_naming_invariants(
            prefix in prop_oneof![
                Just("feat/"),
                Just("fix/"),
                Just("chore/"),
                Just("ci/"),
                Just("docs/"),
                Just("refactor/"),
            ],
            suffix in branch_suffix_regex()
        ) {
            let branch = format!("{prefix}{suffix}");
            prop_assert!(branch_naming_convention(&branch).is_ok());
        }

        #[test]
        fn proptest_path_allowlist_invariants(
            prefix in prop_oneof![Just("kitty-specs/"), Just("docs/")],
            suffix in path_suffix_regex()
        ) {
            prop_assume!(!suffix.contains(".."));
            let path = format!("{prefix}{suffix}");
            prop_assert!(path_allowlist(&path).is_ok());
        }
    }

    #[test]
    fn name_rule_rejects_blank_names() {
        assert!(name_required("").is_err());
        assert!(name_required("   ").is_err());
        assert!(name_required("feature").is_ok());
    }

    #[test]
    fn semver_rule_accepts_three_numeric_parts() {
        assert!(semver_format("1.2.3").is_ok());
        assert!(semver_format("1.2").is_err());
        assert!(semver_format("1.2.x").is_err());
    }

    #[test]
    fn branch_rule_rejects_unknown_prefixes() {
        assert!(branch_naming_convention("main").is_err());
        assert!(branch_naming_convention("release/1.0").is_err());
        assert!(branch_naming_convention("feat/").is_err());
        assert!(branch_naming_convention("feat/demo//bad").is_err());
    }

    #[test]
    fn path_rule_rejects_unsafe_or_out_of_scope_paths() {
        assert!(path_allowlist("../kitty-specs/demo").is_err());
        assert!(path_allowlist("src/main.rs").is_err());
        assert!(path_allowlist("/docs/spec.md").is_err());
    }
}
