// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ecosystem metadata: filename conventions, OSV strings, helpers.
//!
//! Re-exports the canonical [`Ecosystem`] enum from [`crate::dependency`]
//! and adds the filename tables used by [`crate::manifest`] and
//! [`crate::lockfile`] to dispatch parsers.

use std::path::Path;

use crate::dependency::Ecosystem;

pub use crate::dependency::Source;

/// File names we treat as manifests, per ecosystem.
pub const CARGO_MANIFEST: &str = "Cargo.toml";
/// File name we treat as the Cargo lockfile.
pub const CARGO_LOCKFILE: &str = "Cargo.lock";
/// File names we treat as npm manifests.
pub const NPM_MANIFEST: &str = "package.json";
/// File name we treat as the npm lockfile.
pub const NPM_LOCKFILE: &str = "package-lock.json";
/// File names we treat as Python manifests (modern + classic).
pub const PYPI_MANIFEST: &str = "pyproject.toml";
/// File name we treat as the classic PyPI lockfile.
pub const PYPI_LOCKFILE: &str = "requirements.txt";
/// File name we treat as the Go manifest.
pub const GO_MANIFEST: &str = "go.mod";
/// File name we treat as the Go lockfile.
pub const GO_LOCKFILE: &str = "go.sum";

/// Identify the ecosystem that owns a manifest file at `path`.
///
/// Returns `None` if the filename isn't a known manifest. The check is
/// filename-based, not content-based; callers that need to disambiguate
/// (e.g. `pyproject.toml` vs `setup.py`) should pre-filter.
pub fn ecosystem_for_manifest(path: &Path) -> Option<Ecosystem> {
    let name = path.file_name().and_then(|s| s.to_str())?;
    match name {
        CARGO_MANIFEST => Some(Ecosystem::Cargo),
        NPM_MANIFEST => Some(Ecosystem::Npm),
        PYPI_MANIFEST => Some(Ecosystem::Pypi),
        GO_MANIFEST => Some(Ecosystem::Go),
        _ => None,
    }
}

/// Identify the ecosystem that owns a lockfile at `path`.
pub fn ecosystem_for_lockfile(path: &Path) -> Option<Ecosystem> {
    let name = path.file_name().and_then(|s| s.to_str())?;
    match name {
        CARGO_LOCKFILE => Some(Ecosystem::Cargo),
        NPM_LOCKFILE => Some(Ecosystem::Npm),
        PYPI_LOCKFILE => Some(Ecosystem::Pypi),
        GO_LOCKFILE => Some(Ecosystem::Go),
        _ => None,
    }
}

/// True if `path` looks like a manifest we can parse.
pub fn is_manifest(path: &Path) -> bool {
    ecosystem_for_manifest(path).is_some()
}

/// True if `path` looks like a lockfile we can parse.
pub fn is_lockfile(path: &Path) -> bool {
    ecosystem_for_lockfile(path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn manifest_dispatch_table() {
        assert_eq!(
            ecosystem_for_manifest(&PathBuf::from("Cargo.toml")),
            Some(Ecosystem::Cargo)
        );
        assert_eq!(
            ecosystem_for_manifest(&PathBuf::from("package.json")),
            Some(Ecosystem::Npm)
        );
        assert_eq!(
            ecosystem_for_manifest(&PathBuf::from("pyproject.toml")),
            Some(Ecosystem::Pypi)
        );
        assert_eq!(
            ecosystem_for_manifest(&PathBuf::from("go.mod")),
            Some(Ecosystem::Go)
        );
        assert_eq!(ecosystem_for_manifest(&PathBuf::from("setup.py")), None);
        assert_eq!(ecosystem_for_manifest(&PathBuf::from("random.txt")), None);
    }

    #[test]
    fn lockfile_dispatch_table() {
        assert_eq!(
            ecosystem_for_lockfile(&PathBuf::from("Cargo.lock")),
            Some(Ecosystem::Cargo)
        );
        assert_eq!(
            ecosystem_for_lockfile(&PathBuf::from("package-lock.json")),
            Some(Ecosystem::Npm)
        );
        assert_eq!(
            ecosystem_for_lockfile(&PathBuf::from("requirements.txt")),
            Some(Ecosystem::Pypi)
        );
        assert_eq!(
            ecosystem_for_lockfile(&PathBuf::from("go.sum")),
            Some(Ecosystem::Go)
        );
        assert_eq!(ecosystem_for_lockfile(&PathBuf::from("poetry.lock")), None);
    }

    #[test]
    fn is_manifest_and_is_lockfile_are_exhaustive() {
        for f in &["Cargo.toml", "package.json", "pyproject.toml", "go.mod"] {
            assert!(is_manifest(&PathBuf::from(f)), "{f} should be a manifest");
        }
        for f in &[
            "Cargo.lock",
            "package-lock.json",
            "requirements.txt",
            "go.sum",
        ] {
            assert!(is_lockfile(&PathBuf::from(f)), "{f} should be a lockfile");
        }
        assert!(!is_manifest(&PathBuf::from("poetry.lock")));
        assert!(!is_lockfile(&PathBuf::from("Cargo.toml")));
    }
}
