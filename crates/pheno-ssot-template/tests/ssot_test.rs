//! Integration tests for the SSOT trait + render pattern.
//!
//! These tests are intentionally small: the goal is to lock in
//! the structural shape of each renderer (Markdown, JSON,
//! template placeholder) so a refactor that changes the
//! output is forced through a test update — exactly the
//! behavior the SSOT pattern is meant to enforce.

use pheno_ssot_template::{Environment, Render, SSOT, render};

/// A custom `SSOT` impl used to verify the trait is
/// implementable by third parties, not just by the stub
/// `Environment` in the crate.
struct ServiceConfig;

impl SSOT for ServiceConfig {
    fn name(&self) -> &str {
        "billing-api"
    }
    fn section(&self) -> &str {
        "Services"
    }
    fn rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("replicas", "3".to_string()),
            ("image", "ghcr.io/acme/billing:v1.2.0".to_string()),
        ]
    }
}

#[test]
fn markdown_format_emits_section_and_rows() {
    let cfg = ServiceConfig;
    let out = render(&cfg, Render::Markdown);
    assert!(out.contains("## Services"), "missing section heading: {out}");
    assert!(out.contains("**billing-api:**"), "missing name: {out}");
    assert!(
        out.contains("- **replicas:** `3`"),
        "missing replicas row: {out}"
    );
    assert!(
        out.contains("- **image:** `ghcr.io/acme/billing:v1.2.0`"),
        "missing image row: {out}"
    );
}

#[test]
fn json_format_emits_valid_braced_object() {
    let cfg = ServiceConfig;
    let out = render(&cfg, Render::Json);
    assert!(out.starts_with('{'), "json must start with `{{`: {out}");
    assert!(out.ends_with('}'), "json must end with `}}`: {out}");
    assert!(out.contains("\"name\": \"billing-api\""), "{out}");
    assert!(out.contains("\"section\": \"Services\""), "{out}");
    assert!(out.contains("\"replicas\": \"3\""), "{out}");
    assert!(
        out.contains("\"image\": \"ghcr.io/acme/billing:v1.2.0\""),
        "{out}"
    );
}

#[test]
fn template_placeholder_format_uses_double_braces() {
    let cfg = ServiceConfig;
    let out = render(&cfg, Render::TemplatePlaceholder);
    assert!(out.contains("name = {{ name }}"), "{out}");
    assert!(out.contains("section = {{ section }}"), "{out}");
    assert!(out.contains("replicas = {{ replicas }}"), "{out}");
    assert!(out.contains("image = {{ image }}"), "{out}");
}

#[test]
fn render_as_str_returns_stable_lowercase_ids() {
    assert_eq!(Render::Markdown.as_str(), "markdown");
    assert_eq!(Render::Json.as_str(), "json");
    assert_eq!(
        Render::TemplatePlaceholder.as_str(),
        "template-placeholder"
    );
}

#[test]
fn environment_stub_handles_label_and_default_region() {
    let mut env = Environment::ssot("staging");
    env.label = Some("Staging (EU)".to_string());
    env.region = "eu-west-1".to_string();

    let md = env.render(Render::Markdown);
    assert!(md.contains("- **region:** `eu-west-1`"), "{md}");
    assert!(
        md.contains("- **label:** `Staging (EU)`"),
        "label must appear as a row: {md}"
    );
}
