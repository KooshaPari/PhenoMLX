// SPDX-License-Identifier: MIT OR Apache-2.0
//! Manifest parsers for the four supported ecosystems.
//!
//! Each parser turns the raw file contents into a `Vec<Dependency>`. The
//! dispatch is filename-based (see [`ecosystem::ecosystem_for_manifest`])
//! and is exposed as the file-level [`parse_manifest`] entry point.
//!
//! Public convenience surface (string-only, infallible for well-formed
//! input — invalid input is silently skipped rather than panicking):
//! - [`parse_cargo`]      — `Cargo.toml`
//! - [`parse_npm`]        — `package.json`
//! - [`parse_pypi`]       — `pyproject.toml` (PEP 621)
//! - [`parse_go`]         — `go.mod` (single + grouped `require` blocks)
//!
//! Path-level entry point:
//! - [`parse_manifest`]   — reads from disk, dispatches on filename
//!
//! Sections covered:
//! - Cargo: `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`
//! - Npm:   `dependencies`, `devDependencies`, `optionalDependencies`,
//!   `peerDependencies`
//! - PyPI:  `[project.dependencies]`, `[project.optional-dependencies.*]`
//! - Go:    `require ...` lines and `require (...)` blocks

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::dependency::{Dependency, Ecosystem};
use crate::ecosystem;
use crate::error::{Error, Result};
use crate::lockfile::split_requirements_line;

/// Read a manifest from disk and dispatch to the right parser.
///
/// Returns [`Error::Manifest`] for an unsupported filename and propagates
/// IO errors verbatim. (Parse errors are swallowed by the per-ecosystem
/// parsers — they return `Vec::new()` for malformed input.)
pub fn parse_manifest(path: &Path) -> Result<Vec<Dependency>> {
    let ecosystem = ecosystem::ecosystem_for_manifest(path).ok_or_else(|| {
        Error::Manifest(format!("unsupported manifest filename: {}", path.display()))
    })?;
    let raw = fs::read_to_string(path)?;
    let deps = match ecosystem {
        Ecosystem::Cargo => parse_cargo(&raw),
        Ecosystem::Npm => parse_npm(&raw),
        Ecosystem::Pypi => parse_pypi(&raw),
        Ecosystem::Go => parse_go(&raw),
        Ecosystem::Other => Vec::new(),
    };
    Ok(deps)
}

/// Parse the contents of a `Cargo.toml` manifest. Returns one
/// `Dependency` per `[dependencies]`, `[dev-dependencies]`, and
/// `[build-dependencies]` entry. Bare-version and table-with-version
/// forms are both supported.
pub fn parse_cargo(raw: &str) -> Vec<Dependency> {
    let Ok(manifest) = toml::from_str::<CargoToml>(raw) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, spec) in manifest.dependencies {
        out.push(dep_from_cargo_spec(name, spec, "dependencies"));
    }
    for (name, spec) in manifest.dev_dependencies {
        out.push(dep_from_cargo_spec(name, spec, "dev-dependencies"));
    }
    for (name, spec) in manifest.build_dependencies {
        out.push(dep_from_cargo_spec(name, spec, "build-dependencies"));
    }
    out
}

fn dep_from_cargo_spec(name: String, spec: CargoDepSpec, section: &'static str) -> Dependency {
    let version = match spec {
        CargoDepSpec::Bare(v) => v,
        CargoDepSpec::Detailed { version } => version.unwrap_or_else(|| "*".into()),
    };
    Dependency::new(name, version, Ecosystem::Cargo, "Cargo.toml").with_section(section)
}

/// Parse the contents of a `package.json` manifest. Returns one
/// `Dependency` per `dependencies`, `devDependencies`,
/// `optionalDependencies`, and `peerDependencies` entry.
pub fn parse_npm(raw: &str) -> Vec<Dependency> {
    let Ok(pkg) = serde_json::from_str::<NpmPackage>(raw) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, version) in pkg.dependencies {
        out.push(
            Dependency::new(name, version, Ecosystem::Npm, "package.json")
                .with_section("dependencies"),
        );
    }
    for (name, version) in pkg.dev_dependencies {
        out.push(
            Dependency::new(name, version, Ecosystem::Npm, "package.json")
                .with_section("devDependencies"),
        );
    }
    for (name, version) in pkg.optional_dependencies {
        out.push(
            Dependency::new(name, version, Ecosystem::Npm, "package.json")
                .with_section("optionalDependencies"),
        );
    }
    for (name, version) in pkg.peer_dependencies {
        out.push(
            Dependency::new(name, version, Ecosystem::Npm, "package.json")
                .with_section("peerDependencies"),
        );
    }
    out
}

/// Parse the contents of a `pyproject.toml` manifest. Returns one
/// `Dependency` per `[project.dependencies]` entry plus one per
/// `[project.optional-dependencies.<extra>]` entry.
///
/// Auto-detects format: if the input does not look like a TOML
/// `pyproject.toml` (i.e. TOML parsing fails or yields nothing), the
/// function falls back to `requirements.txt` syntax, which makes it
/// safe to call `parse_pypi` on either kind of file.
pub fn parse_pypi(raw: &str) -> Vec<Dependency> {
    let toml_deps = parse_pypi_pyproject(raw);
    if !toml_deps.is_empty() {
        return toml_deps;
    }
    // Fall back to requirements.txt syntax.
    parse_requirements_txt_format(raw)
}

fn parse_pypi_pyproject(raw: &str) -> Vec<Dependency> {
    let Ok(pyp) = toml::from_str::<PyProject>(raw) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(project) = pyp.project {
        for spec in project.dependencies {
            let (name, version) = split_pep508(&spec);
            out.push(
                Dependency::new(name, version, Ecosystem::Pypi, "pyproject.toml")
                    .with_section("project.dependencies"),
            );
        }
        for (name, spec) in project.optional_dependencies {
            for s in spec {
                let (n, v) = split_pep508(&s);
                out.push(
                    Dependency::new(n, v, Ecosystem::Pypi, "pyproject.toml")
                        .with_section(format!("project.optional-dependencies.{name}")),
                );
            }
        }
    }
    out
}

fn parse_requirements_txt_format(raw: &str) -> Vec<Dependency> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("-r ") {
            let target = rest.trim().to_string();
            out.push(
                Dependency::new(target, "*", Ecosystem::Pypi, "requirements.txt")
                    .with_section("-r"),
            );
            continue;
        }
        let (name, version) = split_requirements_line(line);
        if name.is_empty() || name == "*" {
            continue;
        }
        out.push(
            Dependency::new(name, version, Ecosystem::Pypi, "requirements.txt")
                .with_section("requirements"),
        );
    }
    out
}

/// Parse the contents of a `go.mod` file. Handles both the inline
/// `require module version` form and grouped `require (...)` blocks.
/// Indirect markers (`// indirect`) are tolerated but ignored.
pub fn parse_go(raw: &str) -> Vec<Dependency> {
    let mut out = Vec::new();
    let mut in_require = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with("require (") {
            in_require = true;
            continue;
        }
        if in_require && trimmed == ")" {
            in_require = false;
            continue;
        }
        if in_require {
            if let Some((name, version)) = parse_go_require_line(trimmed) {
                out.push(
                    Dependency::new(name, version, Ecosystem::Go, "go.mod").with_section("require"),
                );
            }
            continue;
        }
        if trimmed.starts_with("require ") {
            if let Some(rest) = trimmed.strip_prefix("require ") {
                if let Some((name, version)) = parse_go_require_line(rest.trim()) {
                    out.push(
                        Dependency::new(name, version, Ecosystem::Go, "go.mod")
                            .with_section("require"),
                    );
                }
            }
        }
    }
    out
}

// ---- internal: serde shapes ----------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct CargoToml {
    #[serde(default)]
    dependencies: std::collections::BTreeMap<String, CargoDepSpec>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: std::collections::BTreeMap<String, CargoDepSpec>,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: std::collections::BTreeMap<String, CargoDepSpec>,
}

/// `Cargo.toml` accepts either a bare version string or a table with a
/// `version` key (plus optional `features`, `git`, etc.). We only care
/// about the version field.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CargoDepSpec {
    Bare(String),
    Detailed { version: Option<String> },
}

#[derive(Debug, Default, Deserialize)]
struct NpmPackage {
    #[serde(default)]
    dependencies: std::collections::BTreeMap<String, String>,
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: std::collections::BTreeMap<String, String>,
    #[serde(default, rename = "optionalDependencies")]
    optional_dependencies: std::collections::BTreeMap<String, String>,
    #[serde(default, rename = "peerDependencies")]
    peer_dependencies: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
struct PyProject {
    #[serde(default)]
    project: Option<PyProjectSection>,
}

#[derive(Debug, Default, Deserialize)]
struct PyProjectSection {
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default, rename = "optional-dependencies")]
    optional_dependencies: std::collections::BTreeMap<String, Vec<String>>,
}

// ---- internal: tiny helpers ---------------------------------------------

/// Split a PEP 508 spec like `requests >= 2.0, < 3` into `(name, version)`.
/// Returns `("*", "*")` if we can't find a name.
fn split_pep508(spec: &str) -> (String, String) {
    let mut parts = spec.split_whitespace();
    let name = parts.next().unwrap_or("").to_string();
    let version = parts.next().unwrap_or("*").to_string();
    if name.is_empty() {
        ("*".into(), "*".into())
    } else {
        (name, version)
    }
}

/// Parse a single line inside a `go.mod` `require` block, e.g.
/// `github.com/foo/bar v1.2.3` or `github.com/foo/bar v1.2.3 // indirect`.
/// The trailing `// indirect` marker is stripped; the `v`-prefix on
/// the version is **preserved** (`v1.2.3` stays `v1.2.3`).
fn parse_go_require_line(line: &str) -> Option<(String, String)> {
    let line = line.split("//").next().unwrap_or("").trim();
    let mut parts = line.split_whitespace();
    let name = parts.next()?.to_string();
    let version = parts.next()?.to_string();
    if name.is_empty() || version.is_empty() {
        None
    } else {
        Some((name, version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARGO: &str = r#"
[package]
name = "demo"
version = "0.0.1"

[dependencies]
serde = "1.0"
tokio = { version = "1.46", features = ["full"] }

[dev-dependencies]
tempfile = "3"
"#;

    const NPM: &str = r#"
{
  "name": "demo",
  "version": "0.0.1",
  "dependencies": { "lodash": "4.17.21" },
  "devDependencies": { "jest": "29.0.0" },
  "optionalDependencies": { "fsevents": "2.3.0" },
  "peerDependencies": { "react": "18.0.0" }
}
"#;

    const PYPI: &str = r#"
[project]
name = "demo"
version = "0.0.1"
dependencies = [
  "requests >= 2.0",
  "click",
]

[project.optional-dependencies]
gui = ["PyQt5"]
"#;

    const GO: &str = r#"
module example.com/demo

go 1.22

require github.com/foo/bar v1.2.3

require (
    github.com/baz/qux v0.4.0
    github.com/deep/nest v2.0.1 // indirect
)
"#;

    #[test]
    fn cargo_manifest_parses_all_sections() {
        let deps = parse_cargo(CARGO);
        assert_eq!(deps.len(), 3);
        let serde = deps.iter().find(|d| d.name == "serde").unwrap();
        assert_eq!(serde.version, "1.0");
        assert_eq!(serde.source.section.as_deref(), Some("dependencies"));
        let tokio = deps.iter().find(|d| d.name == "tokio").unwrap();
        assert_eq!(tokio.version, "1.46");
        let tmp = deps.iter().find(|d| d.name == "tempfile").unwrap();
        assert_eq!(tmp.source.section.as_deref(), Some("dev-dependencies"));
    }

    #[test]
    fn cargo_manifest_with_no_dev_dependencies() {
        let raw = r#"
[package]
name = "x"
version = "0.0.1"
[dependencies]
serde = "1"
"#;
        let deps = parse_cargo(raw);
        assert_eq!(deps.len(), 1);
    }

    #[test]
    fn npm_manifest_parses_all_sections() {
        let deps = parse_npm(NPM);
        assert_eq!(deps.len(), 4);
        let lodash = deps.iter().find(|d| d.name == "lodash").unwrap();
        assert_eq!(lodash.version, "4.17.21");
        assert_eq!(lodash.source.section.as_deref(), Some("dependencies"));
        let jest = deps.iter().find(|d| d.name == "jest").unwrap();
        assert_eq!(jest.source.section.as_deref(), Some("devDependencies"));
        let fse = deps.iter().find(|d| d.name == "fsevents").unwrap();
        assert_eq!(fse.source.section.as_deref(), Some("optionalDependencies"));
        let react = deps.iter().find(|d| d.name == "react").unwrap();
        assert_eq!(react.source.section.as_deref(), Some("peerDependencies"));
    }

    #[test]
    fn pypi_manifest_parses_pep508() {
        let deps = parse_pypi(PYPI);
        assert!(deps
            .iter()
            .any(|d| d.name == "requests" && d.version == ">="));
        assert!(deps.iter().any(|d| d.name == "click" && d.version == "*"));
        assert!(deps.iter().any(|d| d.name == "PyQt5"));
    }

    #[test]
    fn go_manifest_parses_both_require_forms() {
        let deps = parse_go(GO);
        assert_eq!(deps.len(), 3);
        let bar = deps
            .iter()
            .find(|d| d.name == "github.com/foo/bar")
            .unwrap();
        assert_eq!(bar.version, "v1.2.3");
        let qux = deps
            .iter()
            .find(|d| d.name == "github.com/baz/qux")
            .unwrap();
        assert_eq!(qux.version, "v0.4.0");
        let nest = deps
            .iter()
            .find(|d| d.name == "github.com/deep/nest")
            .unwrap();
        assert_eq!(nest.version, "v2.0.1");
    }

    #[test]
    fn go_manifest_ignores_indirect_marker() {
        let deps = parse_go(GO);
        assert_eq!(deps.iter().filter(|d| d.version == "v2.0.1").count(), 1);
    }

    #[test]
    fn unsupported_manifest_filename_errors() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("setup.py");
        std::fs::write(&p, "from setuptools import setup").unwrap();
        let err = parse_manifest(&p).unwrap_err();
        assert!(err.to_string().contains("unsupported manifest"));
    }

    #[test]
    fn split_pep508_with_version() {
        let (n, v) = split_pep508("requests >= 2.0");
        assert_eq!(n, "requests");
        assert_eq!(v, ">=");
    }

    #[test]
    fn split_pep508_without_version() {
        let (n, v) = split_pep508("click");
        assert_eq!(n, "click");
        assert_eq!(v, "*");
    }
}
