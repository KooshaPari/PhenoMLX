// SPDX-License-Identifier: MIT OR Apache-2.0
//! Unit + wiremock integration tests.

use std::fs;

use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use phenotype_dep_guard::{
    Dependency, Ecosystem, OsvClient, Sbom, Scanner, ScannerConfig, Severity,
};

fn dep(name: &str, version: &str) -> Dependency {
    Dependency::new(name, version, Ecosystem::Cargo, "Cargo.toml")
}

#[test]
fn ecosystem_serializes_lowercase() {
    assert_eq!(
        serde_json::to_string(&Ecosystem::Cargo).unwrap(),
        "\"cargo\""
    );
    assert_eq!(Ecosystem::Cargo.as_osv_str(), "crates.io");
    assert_eq!(Ecosystem::Npm.as_osv_str(), "npm");
    assert_eq!(Ecosystem::Pypi.as_osv_str(), "PyPI");
}

#[test]
fn severity_buckets_from_cvss() {
    assert_eq!(Severity::from_cvss(0.0), Severity::Unknown);
    assert_eq!(Severity::from_cvss(2.5), Severity::Low);
    assert_eq!(Severity::from_cvss(5.0), Severity::Medium);
    assert_eq!(Severity::from_cvss(7.5), Severity::High);
    assert_eq!(Severity::from_cvss(9.5), Severity::Critical);
}

#[test]
fn sbom_contains_components_and_serializes() {
    let deps = vec![dep("serde", "1.0.0"), dep("tokio", "1.46.0")];
    let sbom = Sbom::new("phenotype-dep-guard", "0.1.0", &deps);
    let json = sbom.to_json().unwrap();
    assert!(json.contains("\"bomFormat\":\"CycloneDX\""));
    assert!(json.contains("\"specVersion\":\"1.5\""));
    assert!(json.contains("\"pkg:cargo/serde@1.0.0\""));
    assert!(json.contains("\"pkg:cargo/tokio@1.46.0\""));
    assert_eq!(sbom.components.len(), 2);
    assert_eq!(sbom.metadata.component.name, "phenotype-dep-guard");
}

#[test]
fn scanner_parses_cargo_manifest() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Cargo.toml");
    fs::write(
        &path,
        r#"
[package]
name = "demo"
version = "0.0.1"

[dependencies]
serde = "1.0"
tokio = "1.46"

[dev-dependencies]
tempfile = "3"

[build-dependencies]
nothing = "0.0"
"#,
    )
    .unwrap();

    let deps = Scanner::parse_cargo_manifest(&path).unwrap();
    assert_eq!(deps.len(), 4);
    let serde = deps.iter().find(|d| d.name == "serde").unwrap();
    assert_eq!(serde.version, "1.0");
    assert_eq!(serde.source.section.as_deref(), Some("dependencies"));
    let tmp = deps.iter().find(|d| d.name == "tempfile").unwrap();
    assert_eq!(tmp.source.section.as_deref(), Some("dev-dependencies"));
}

#[tokio::test]
async fn osv_query_via_wiremock() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "vulns": [{
                "id": "RUSTSEC-2024-0001",
                "summary": "Demo vuln in some crate",
                "published": "2024-01-15T00:00:00Z",
                "severity": [{ "type": "CVSS_V3", "score": "8.5" }],
                "affected": [{
                    "ranges": [{
                        "events": [
                            { "introduced": "0" },
                            { "fixed": "1.2.0" }
                        ]
                    }]
                }]
            }]
        })))
        .mount(&server)
        .await;

    let client = OsvClient::with_endpoint(server.uri()).unwrap();
    let vulns = client.query(&dep("some-crate", "1.0.0")).await.unwrap();
    assert_eq!(vulns.len(), 1);
    let v = &vulns[0];
    assert_eq!(v.id, "RUSTSEC-2024-0001");
    assert_eq!(v.severity, Severity::High);
    assert!(v.matches, "1.0.0 should fall in [0, 1.2.0)");
    assert!(v.affected_ranges[0].contains("[0"));

    // Out-of-range version: not a match.
    let vulns2 = client.query(&dep("some-crate", "1.2.5")).await.unwrap();
    assert!(!vulns2[0].matches);
}

#[tokio::test]
async fn scanner_full_round_trip_against_wiremock() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/query"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "vulns": [] })))
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("Cargo.toml");
    fs::write(
        &path,
        r#"
[package]
name = "x"
version = "0.0.1"
[dependencies]
serde = "1.0"
"#,
    )
    .unwrap();

    let scanner = Scanner::new(
        ScannerConfig::new("x", "0.0.1").with_osv(OsvClient::with_endpoint(server.uri()).unwrap()),
    );
    let (report, sbom) = scanner.scan_cargo_manifest(&path).await.unwrap();
    assert_eq!(report.summary.total, 1);
    assert_eq!(report.summary.with_findings, 0);
    assert_eq!(report.summary.critical, 0);
    assert_eq!(sbom.components.len(), 1);
}

#[test]
fn report_blocks_on_critical() {
    use phenotype_dep_guard::report::{Finding, Report};
    use phenotype_dep_guard::vulnerability::Vulnerability;

    let v = Vulnerability {
        id: "X-1".into(),
        summary: "x".into(),
        severity: Severity::Critical,
        affected_ranges: vec![],
        published: None,
        matches: true,
    };
    let finding = Finding {
        dependency: dep("x", "0.0.1"),
        vulnerabilities: vec![v],
    };
    let report = Report::from_findings(vec![finding]);
    assert!(report.has_blocking_findings());
    assert_eq!(report.summary.critical, 1);
}
