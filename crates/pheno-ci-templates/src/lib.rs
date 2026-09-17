//! # pheno-ci-templates — reusable GitHub Actions templates
//!
//! This crate is *not* a runtime library. It exists to give
//! the workspace a single, versioned source of truth for
//! the GitHub Actions workflows that the monorepo's
//! services include via:
//!
//! ```yaml
//! uses: agileplus-ai/pheno-ci-templates/.github/workflows/ci-rust.yml@v0.1
//! ```
//!
//! The `Cargo.toml` exists so the crate is visible in the
//! workspace (`cargo metadata`, `cargo xtask list`, etc.)
//! and so a Rust-side test can lock the workflow file
//! layout against drift. The `lib.rs` is intentionally
//! minimal — the deliverable is the YAML under
//! `.github/workflows/`.
//!
//! ## Layout
//!
//! - `ci-base.yml`    — checkout + concurrency settings,
//!   `workflow_call`-only.
//! - `ci-rust.yml`    — adds `cargo fmt` / `clippy` / `test`.
//! - `ci-node.yml`    — adds `pnpm install` / `tsc` / `vitest`.
//! - `ci-python.yml`  — adds `uv sync` / `ruff` / `pytest`.
//!
//! ## Why a crate?
//!
//! The four YAML files are versioned and distributed via
//! the crate's git tag. A consuming repo pins the tag
//! (`@v0.1`) and gets a known-good, reviewable set of
//! steps. Changing the cache key or the checkout depth
//! happens in one place.

/// Filesystem path (relative to the crate root) of the
/// `ci-base.yml` reusable workflow. Useful for tooling
/// that wants to assert the file exists at a known
/// location.
pub const CI_BASE_PATH: &str = ".github/workflows/ci-base.yml";

/// Filesystem path of the `ci-rust.yml` reusable workflow.
pub const CI_RUST_PATH: &str = ".github/workflows/ci-rust.yml";

/// Filesystem path of the `ci-node.yml` reusable workflow.
pub const CI_NODE_PATH: &str = ".github/workflows/ci-node.yml";

/// Filesystem path of the `ci-python.yml` reusable workflow.
pub const CI_PYTHON_PATH: &str = ".github/workflows/ci-python.yml";

/// All four paths, in the order the include chain should
/// be consumed (`ci-base` first, language-specific after).
pub const ALL_PATHS: &[&str] = &[CI_BASE_PATH, CI_RUST_PATH, CI_NODE_PATH, CI_PYTHON_PATH];

/// Returns `true` if every file in [`ALL_PATHS`] is
/// present at the path it claims. The check is
/// filesystem-based so it can be used both in a unit
/// test (this file IS the unit test) and at runtime
/// from an `xtask` command.
pub fn all_present() -> bool {
    // `CARGO_MANIFEST_DIR` is set by Cargo at build time
    // and points to this crate's root. We resolve each
    // path relative to it.
    let manifest = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(s) => s,
        Err(_) => return false,
    };
    ALL_PATHS
        .iter()
        .all(|p| std::path::Path::new(&manifest).join(p).is_file())
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    /// Inline smoke test: the four constant paths are
    /// non-empty and all start with `.github/workflows/`.
    /// The integration test in `tests/include_test.rs`
    /// verifies the files actually exist on disk.
    #[test]
    fn constants_are_well_formed() {
        for p in ALL_PATHS {
            assert!(!p.is_empty(), "constant path must not be empty");
            assert!(
                p.starts_with(".github/workflows/"),
                "constant path must be under `.github/workflows/`, got {p}",
            );
        }
    }
}
