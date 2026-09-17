// SPDX-License-Identifier: MIT OR Apache-2.0
//! Dependency model.
//!
//! A `Dependency` is a single line in a manifest after normalization:
//! `{name, version, ecosystem, source}`. Source carries provenance so the
//! scanner can produce a per-manifest SBOM component entry.

use serde::{Deserialize, Serialize};

/// Package ecosystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    /// Rust crates (Cargo).
    Cargo,
    /// Node packages (npm / yarn / pnpm).
    Npm,
    /// Python packages (PyPI).
    Pypi,
    /// Go modules.
    Go,
    /// Other / unknown.
    Other,
}

impl Ecosystem {
    /// OSV.dev ecosystem name (matches their `package.ecosystem` enum).
    pub fn as_osv_str(self) -> &'static str {
        match self {
            Ecosystem::Cargo => "crates.io",
            Ecosystem::Npm => "npm",
            Ecosystem::Pypi => "PyPI",
            Ecosystem::Go => "Go",
            Ecosystem::Other => "Other",
        }
    }
}

/// Where the dependency was declared.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    /// Manifest file path, relative to the repo root.
    pub manifest: String,
    /// Optional section/feature hint (e.g. `dependencies`, `dev-dependencies`,
    /// `[features.foo]`, `[workspace.dependencies]`).
    pub section: Option<String>,
}

/// A normalized dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Package name as it appears in the manifest.
    pub name: String,
    /// Resolved version, if known. May be `"*"` for unrestricted ranges.
    pub version: String,
    /// Ecosystem.
    pub ecosystem: Ecosystem,
    /// Provenance.
    pub source: Source,
}

impl Dependency {
    /// Construct a new dependency.
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        ecosystem: Ecosystem,
        manifest: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            ecosystem,
            source: Source {
                manifest: manifest.into(),
                section: None,
            },
        }
    }

    /// Attach a section hint.
    pub fn with_section(mut self, section: impl Into<String>) -> Self {
        self.source.section = Some(section.into());
        self
    }
}
