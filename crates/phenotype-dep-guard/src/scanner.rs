// SPDX-License-Identifier: MIT OR Apache-2.0
//! Top-level scanner that wires dependency parsing + OSV + SBOM together.

use std::path::Path;

use crate::dependency::{Dependency, Ecosystem};
use crate::error::{Error, Result};
use crate::osv::OsvClient;
use crate::report::{Finding, Report};
use crate::sbom::Sbom;

/// Scanner configuration.
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    /// Project name (used as SBOM root component name).
    pub name: String,
    /// Project version.
    pub version: String,
    /// Optional pre-built OSV client (override endpoint for tests).
    pub osv: Option<OsvClient>,
}

impl ScannerConfig {
    /// Build a config with a fresh OSV client.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            osv: None,
        }
    }

    /// Override the OSV client.
    pub fn with_osv(mut self, client: OsvClient) -> Self {
        self.osv = Some(client);
        self
    }
}

/// Builder for [`Scanner`] with fluent configuration.
///
/// Use this when you need to specify multiple paths, ecosystems, or
/// non-default OSV settings. For single-call use, [`Scanner::new`]
/// + [`Scanner::scan`] is sufficient.
#[derive(Debug, Clone)]
pub struct ScannerBuilder {
    config: ScannerConfig,
    paths: Vec<std::path::PathBuf>,
    ecosystems: Vec<crate::dependency::Ecosystem>,
}

impl ScannerBuilder {
    /// Start a new builder. Equivalent to `Scanner::new(ScannerConfig::new(...))`.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            config: ScannerConfig::new(name, version),
            paths: Vec::new(),
            ecosystems: Vec::new(),
        }
    }

    /// Override the OSV client (for tests or private mirrors).
    pub fn osv(mut self, client: OsvClient) -> Self {
        self.config.osv = Some(client);
        self
    }

    /// Restrict scanning to the given ecosystems. Empty = all ecosystems.
    pub fn ecosystems(
        mut self,
        ecosystems: impl IntoIterator<Item = crate::dependency::Ecosystem>,
    ) -> Self {
        self.ecosystems = ecosystems.into_iter().collect();
        self
    }

    /// Add a project root to scan. All recognized manifests under the root are picked up.
    pub fn add_path(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.paths.push(path.into());
        self
    }

    /// Build the [`Scanner`].
    pub fn build(self) -> Scanner {
        Scanner {
            config: self.config,
            paths: self.paths,
            ecosystem_filter: self.ecosystems,
        }
    }
}

/// Result of a multi-path scan: report + SBOM + per-path metadata.
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Aggregated vulnerability report across all paths.
    pub report: Report,
    /// Generated SBOM (CycloneDX).
    pub sbom: Sbom,
    /// Per-path dependency count (path -> count).
    pub per_path: Vec<(std::path::PathBuf, usize)>,
}

impl ScanResult {
    /// Total dependencies scanned (sum across all paths).
    pub fn dependency_count(&self) -> usize {
        self.per_path.iter().map(|(_, n)| *n).sum()
    }
}

/// Orchestrates dependency parsing + OSV scan + report assembly.
#[derive(Debug, Clone)]
pub struct Scanner {
    config: ScannerConfig,
    paths: Vec<std::path::PathBuf>,
    ecosystem_filter: Vec<crate::dependency::Ecosystem>,
}

impl Scanner {
    /// Build a scanner from config.
    pub fn new(config: ScannerConfig) -> Self {
        Self {
            config,
            paths: Vec::new(),
            ecosystem_filter: Vec::new(),
        }
    }

    /// Start a builder for richer configuration.
    pub fn builder(name: impl Into<String>, version: impl Into<String>) -> ScannerBuilder {
        ScannerBuilder::new(name, version)
    }

    /// Add a project root to scan (consumes and returns self for chaining).
    pub fn with_path(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.paths.push(path.into());
        self
    }

    /// Restrict scanning to the given ecosystems (consumes and returns self for chaining).
    pub fn with_ecosystems(
        mut self,
        ecosystems: impl IntoIterator<Item = crate::dependency::Ecosystem>,
    ) -> Self {
        self.ecosystem_filter = ecosystems.into_iter().collect();
        self
    }

    /// Parse dependencies from a `Cargo.toml` manifest at `path`.
    pub fn parse_cargo_manifest(path: impl AsRef<Path>) -> Result<Vec<Dependency>> {
        let raw = std::fs::read_to_string(&path)?;
        let manifest: CargoManifest = toml::from_str(&raw)?;
        let manifest_path = path
            .as_ref()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Cargo.toml")
            .to_string();
        let mut out = Vec::new();
        for (name, ver) in manifest.dependencies {
            out.push(
                Dependency::new(name, ver, Ecosystem::Cargo, &manifest_path)
                    .with_section("dependencies"),
            );
        }
        for (name, ver) in manifest.dev_dependencies {
            out.push(
                Dependency::new(name, ver, Ecosystem::Cargo, &manifest_path)
                    .with_section("dev-dependencies"),
            );
        }
        for (name, ver) in manifest.build_dependencies {
            out.push(
                Dependency::new(name, ver, Ecosystem::Cargo, &manifest_path)
                    .with_section("build-dependencies"),
            );
        }
        Ok(out)
    }

    /// Scan a slice of pre-collected dependencies. Returns a [`Report`].
    pub async fn scan(&self, deps: &[Dependency]) -> Result<Report> {
        let client = self.config.osv.clone().unwrap_or_default();
        let results = client.query_batch(deps).await?;
        let findings = results
            .into_iter()
            .map(|(dep, vulns)| Finding {
                dependency: dep,
                vulnerabilities: vulns,
            })
            .collect();
        Ok(Report::from_findings(findings))
    }

    /// Scan multiple project roots configured on this scanner. Returns a [`ScanResult`].
    ///
    /// Walks each path, parses every recognized manifest (Cargo.toml, package.json,
    /// requirements.txt, go.mod), filters by the configured ecosystems (if any),
    /// then runs a single batched OSV query.
    pub async fn scan_paths(&self) -> Result<ScanResult> {
        use crate::manifest::parse_manifest;
        let mut all_deps: Vec<Dependency> = Vec::new();
        let mut per_path: Vec<(std::path::PathBuf, usize)> = Vec::new();

        for path in &self.paths {
            let path_deps = parse_manifest(path)?;
            let count_before = all_deps.len();
            for dep in path_deps {
                if self.ecosystem_filter.is_empty()
                    || self.ecosystem_filter.contains(&dep.ecosystem)
                {
                    all_deps.push(dep);
                }
            }
            per_path.push((path.clone(), all_deps.len() - count_before));
        }

        let report = self.scan(&all_deps).await?;
        let sbom = Sbom::new(&self.config.name, &self.config.version, &all_deps);
        Ok(ScanResult {
            report,
            sbom,
            per_path,
        })
    }

    /// Scan a Cargo.toml manifest in one call. Returns report + SBOM.
    pub async fn scan_cargo_manifest(&self, path: impl AsRef<Path>) -> Result<(Report, Sbom)> {
        let deps = Self::parse_cargo_manifest(&path)?;
        let report = self.scan(&deps).await?;
        let sbom = Sbom::new(&self.config.name, &self.config.version, &deps);
        Ok((report, sbom))
    }
}

/// Minimal subset of `Cargo.toml` we parse. Anything more elaborate
/// (target-specific, features) is out of scope for the first cut.
#[derive(Debug, serde::Deserialize)]
struct CargoManifest {
    #[serde(default)]
    dependencies: std::collections::BTreeMap<String, String>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: std::collections::BTreeMap<String, String>,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: std::collections::BTreeMap<String, String>,
}

#[allow(dead_code)] // suppresses unused_variables warning for Error match arm - used by macro-generated code
fn _suppress_unused_error(_: Error) {}
