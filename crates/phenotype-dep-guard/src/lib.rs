// SPDX-License-Identifier: MIT OR Apache-2.0
//! Phenotype Dependency Guard
//!
//! Dependency scanning, OSV vulnerability lookup, and CycloneDX SBOM generation
//! across four ecosystems (Cargo, Npm, PyPI, Go).
//!
//! # Quick start
//!
//! ```no_run
//! use phenotype_dep_guard::{OsvClient, ScannerBuilder};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let scanner = ScannerBuilder::new("phenotype-dep-guard", "0.1.0")
//!     .add_path(".")
//!     .osv(OsvClient::default())
//!     .build();
//! let report = scanner.scan_paths().await?.report;
//! println!("{}", report.summary);
//! # Ok(()) }
//! ```
//!
//! # Architecture
//!
//! - [`ecosystem::Ecosystem`] enumerates the four supported ecosystems
//! - [`manifest::parse_manifest`] and [`lockfile::parse_lockfile`] turn files
//!   into [`dependency::Dependency`] records
//! - [`osv::OsvClient`] queries the OSV.dev database for known vulnerabilities
//! - [`sbom::Sbom`] renders the result as a CycloneDX 1.5 JSON document
//! - [`scanner::Scanner`] / [`scanner::ScannerBuilder`] tie it all together
//! - [`report::Report`] is the user-facing summary with severity counts

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms)]
#![warn(unreachable_pub)]

pub mod dependency;
pub mod ecosystem;
pub mod error;
pub mod lockfile;
pub mod manifest;
pub mod osv;
pub mod report;
pub mod sbom;
pub mod scanner;
pub mod vulnerability;

pub use dependency::{Dependency, Ecosystem, Source};
pub use error::{Error, Result};
pub use osv::OsvClient;
pub use report::{Finding, Report, Summary as SeverityCounts};
pub use sbom::{Component as SbomComponent, Sbom, SbomFormat};
pub use scanner::{ScanResult, Scanner, ScannerBuilder, ScannerConfig};
pub use vulnerability::{Severity, Vulnerability};

/// Re-export of the string-only manifest parsers from [`manifest`].
/// These are useful in tests and for parsing `&str` content directly
/// (without going through the filesystem).
pub use manifest::{parse_cargo, parse_go, parse_npm, parse_pypi};

/// Re-export of the string-only lockfile parsers from [`lockfile`].
pub use lockfile::{parse_cargo_lock, parse_go_sum, parse_npm_lock, parse_requirements_txt};

/// Convenience alias so callers can write `DepGuardError` instead of `Error`.
///
/// Both names are valid; this is purely ergonomic.
pub type DepGuardError = Error;

/// Parse a manifest at `path` into a stream of [`Dependency`] records.
///
/// Dispatches on file name (e.g. `Cargo.toml` â†’ Cargo parser, `package.json`
/// â†’ Npm parser). See [`manifest`] for per-ecosystem details.
pub fn parse_manifest(path: &std::path::Path) -> Result<Vec<Dependency>> {
    manifest::parse_manifest(path)
}

/// Parse a lockfile at `path` into a stream of [`Dependency`] records.
///
/// Dispatches on file name (e.g. `Cargo.lock` â†’ Cargo parser,
/// `package-lock.json` â†’ Npm parser, `requirements.txt` â†’ PyPI parser,
/// `go.sum` â†’ Go parser). See [`lockfile`] for per-ecosystem details.
pub fn parse_lockfile(path: &std::path::Path) -> Result<Vec<Dependency>> {
    lockfile::parse_lockfile(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_surface_compiles() {
        // Lock the public surface so accidental renames fail here, not downstream.
        let _: Ecosystem = Ecosystem::Cargo;
        let _: std::result::Result<Report, Error> = Ok(Report::from_findings(vec![]));
        let _: Sbom = Sbom::new("demo", "0.0.1", &[]);
    }

    #[test]
    fn dep_guard_error_alias_matches() {
        // The alias must point to the same type.
        let err: DepGuardError = Error::Other("x".into());
        let _err2: Error = err;
    }

    #[test]
    fn parse_manifest_dispatches_by_filename() {
        // Cargo.toml
        let cargo = parse_cargo(
            r#"
[package]
name = "demo"
version = "0.0.1"
[dependencies]
serde = "1.0"
"#,
        );
        assert_eq!(cargo.len(), 1);
        assert_eq!(cargo[0].name, "serde");
        assert_eq!(cargo[0].ecosystem, Ecosystem::Cargo);

        // package.json
        let npm = parse_npm(
            r#"{
  "name": "demo",
  "version": "0.0.1",
  "dependencies": { "lodash": "^4.0.0" }
}"#,
        );
        assert_eq!(npm.len(), 1);
        assert_eq!(npm[0].name, "lodash");
        assert_eq!(npm[0].ecosystem, Ecosystem::Npm);

        // requirements.txt
        let pypi = parse_pypi("flask==2.0.0\nrequests>=2.28\n# comment\n");
        assert_eq!(pypi.len(), 2);
        assert_eq!(pypi[0].name, "flask");
        assert_eq!(pypi[0].version, "2.0.0");
        assert_eq!(pypi[0].ecosystem, Ecosystem::Pypi);

        // go.mod
        let go = parse_go(
            "module example.com/demo\n\ngo 1.21\n\nrequire github.com/gin-gonic/gin v1.9.0\n",
        );
        assert_eq!(go.len(), 1);
        assert_eq!(go[0].name, "github.com/gin-gonic/gin");
        assert_eq!(go[0].version, "v1.9.0");
        assert_eq!(go[0].ecosystem, Ecosystem::Go);
    }

    #[test]
    fn parse_lockfile_dispatches_by_filename() {
        // Cargo.lock
        let cargo_lock = parse_cargo_lock(
            r#"
version = 3

[[package]]
name = "serde"
version = "1.0.0"

[[package]]
name = "tokio"
version = "1.46.0"
"#,
        );
        assert_eq!(cargo_lock.len(), 2);
        assert!(cargo_lock
            .iter()
            .any(|d| d.name == "serde" && d.version == "1.0.0"));

        // package-lock.json
        let npm_lock = parse_npm_lock(
            r#"{
  "lockfileVersion": 3,
  "packages": {
    "node_modules/lodash": { "version": "4.17.21" },
    "node_modules/express": { "version": "4.18.0" }
  }
}"#,
        );
        assert_eq!(npm_lock.len(), 2);

        // requirements.txt (same format as manifest for PyPI)
        let req = parse_requirements_txt("django==4.2\ncelery>=5.3\n");
        assert_eq!(req.len(), 2);

        // go.sum (one line per direct + one per indirect; both should be parsed)
        let go_sum = parse_go_sum(
            "github.com/gin-gonic/gin v1.9.0 h1:abc=\n\
             github.com/gin-gonic/gin v1.9.0/go-mod h1:def=\n\
             github.com/stretchr/testify v1.8.0 h1:ghi=\n",
        );
        assert_eq!(go_sum.len(), 2);
    }
}
