// SPDX-License-Identifier: MIT OR Apache-2.0
//! CycloneDX-style SBOM representation + JSON export.
//!
//! Implements the minimal CycloneDX 1.5 JSON shape that vulnerability
//! scanners, dependency-track, and `bomctl` all consume. Spec:
//! <https://cyclonedx.org/docs/1.5/json/>

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dependency::Dependency;

/// SBOM output format. Currently only JSON; CycloneDX XML is a follow-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbomFormat {
    /// CycloneDX 1.5 JSON.
    CycloneDxJson,
}

/// One component in the SBOM (a dependency, a library, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    /// CycloneDX component type (always `library` for our deps).
    #[serde(rename = "type")]
    pub kind: String,
    /// Component name (matches `Dependency::name`).
    pub name: String,
    /// Component version (matches `Dependency::version`).
    pub version: String,
    /// CycloneDX package URL (`pkg:cargo/<name>@<version>`).
    pub purl: String,
    /// Optional external references (registry homepage).
    #[serde(
        rename = "externalReferences",
        skip_serializing_if = "Vec::is_empty",
        default
    )]
    pub external_references: Vec<ExternalRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalRef {
    /// `website`, `vcs`, `documentation`, etc.
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
}

/// A CycloneDX 1.5 JSON SBOM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sbom {
    /// CycloneDX spec version.
    #[serde(rename = "bomFormat")]
    pub bom_format: String,
    #[serde(rename = "specVersion")]
    pub spec_version: String,
    pub version: u32,
    /// ISO-8601 serial timestamp.
    #[serde(rename = "serialNumber")]
    pub serial_number: String,
    pub metadata: SbomMetadata,
    pub components: Vec<Component>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SbomMetadata {
    pub timestamp: DateTime<Utc>,
    pub tools: Vec<SbomTool>,
    pub component: SbomRootComponent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SbomTool {
    pub vendor: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SbomRootComponent {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub version: String,
}

impl Sbom {
    /// Build a new SBOM from a list of dependencies, rooted at `root_name`
    /// (typically the product name).
    pub fn new(
        root_name: impl Into<String>,
        root_version: impl Into<String>,
        deps: &[Dependency],
    ) -> Self {
        let components = deps
            .iter()
            .map(|d| Component {
                kind: "library".into(),
                name: d.name.clone(),
                version: d.version.clone(),
                purl: purl_for(d),
                external_references: Vec::new(),
            })
            .collect();
        Self {
            bom_format: "CycloneDX".into(),
            spec_version: "1.5".into(),
            version: 1,
            serial_number: format!("urn:uuid:{}", Uuid::new_v4()),
            metadata: SbomMetadata {
                timestamp: Utc::now(),
                tools: vec![SbomTool {
                    vendor: "Phenotype".into(),
                    name: "phenotype-dep-guard".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                }],
                component: SbomRootComponent {
                    kind: "application".into(),
                    name: root_name.into(),
                    version: root_version.into(),
                },
            },
            components,
        }
    }

    /// Serialize as CycloneDX 1.5 JSON (compact, no whitespace).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

fn purl_for(d: &Dependency) -> String {
    let ecosystem = match d.ecosystem {
        crate::dependency::Ecosystem::Cargo => "cargo",
        crate::dependency::Ecosystem::Npm => "npm",
        crate::dependency::Ecosystem::Pypi => "pypi",
        crate::dependency::Ecosystem::Go => "golang",
        crate::dependency::Ecosystem::Other => "generic",
    };
    format!("pkg:{}/{}@{}", ecosystem, d.name, d.version)
}
