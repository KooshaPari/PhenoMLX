//! # pheno-ssot-template — SSOT trait + struct-and-render pattern
//!
//! A [`SSOT`] value is a typed, self-describing record that knows how
//! to render itself in a few stable formats. The point is to keep
//! a single source of truth (the struct) and avoid the
//! copy-paste drift that happens when the same value is restated
//! in Markdown, JSON, and CI config by hand.
//!
//! ## Quick start
//!
//! ```
//! use pheno_ssot_template::{SSOT, Render, Environment};
//!
//! let env = Environment::ssot("prod");
//! let md = env.render(Render::Markdown);
//! assert!(md.contains("**env:** `prod`"), "markdown: {md}");
//! ```

use std::fmt::Write;

/// A typed, self-describing record that knows how to render itself
/// in multiple stable formats.
///
/// Implementors describe themselves with a small DSL of
/// `(key, value)` rows, plus a `name()` and an optional
/// `section()`. The renderers then take that description and
/// produce a string in the chosen format — no string
/// concatenation in the implementor's body, no copy-paste
/// between formats.
pub trait SSOT {
    /// Human-readable name of the record (e.g. `"prod env"`).
    fn name(&self) -> &str;

    /// Optional section grouping, used by [`Render::Markdown`]
    /// to nest rows under a heading. `""` means top-level.
    fn section(&self) -> &str {
        ""
    }

    /// The rows that make up the record. The first element of
    /// each tuple is the key (a stable identifier), the second
    /// is the value (already a string — callers format
    /// numbers, paths, etc. before returning).
    fn rows(&self) -> Vec<(&'static str, String)>;

    /// Render `self` in `format`. Convenience wrapper around
    /// the free function [`render`] so callers can write
    /// `my_value.render(Render::Markdown)` instead of
    /// `render(&my_value, Render::Markdown)`.
    fn render(&self, format: Render) -> String
    where
        Self: Sized,
    {
        render(self, format)
    }
}

/// Output format selector for [`SSOT::render`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Render {
    /// `**key:** \`value\`` per row, sectioned by `##`.
    Markdown,
    /// `{"key": "value", ...}` JSON object.
    Json,
    /// `{{ key }}` placeholder per line, for use in a
    /// `handlebars` / `minijinja` template.
    TemplatePlaceholder,
}

impl Render {
    /// Stable, lowercase identifier — useful for filenames and
    /// CLI args.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Json => "json",
            Self::TemplatePlaceholder => "template-placeholder",
        }
    }
}

/// Render any [`SSOT`] in the chosen format. Free function so
/// implementors don't have to thread a `&self` through every
/// format.
pub fn render<S: SSOT + ?Sized>(ssot: &S, format: Render) -> String {
    match format {
        Render::Markdown => render_markdown(ssot),
        Render::Json => render_json(ssot),
        Render::TemplatePlaceholder => render_template_placeholder(ssot),
    }
}

fn render_markdown<S: SSOT + ?Sized>(ssot: &S) -> String {
    let mut out = String::new();
    let section = ssot.section();
    if !section.is_empty() {
        let _ = writeln!(out, "## {section}");
    }
    let _ = writeln!(out, "**{name}:**", name = ssot.name());
    for (k, v) in ssot.rows() {
        let _ = writeln!(out, "- **{k}:** `{v}`");
    }
    out
}

fn render_json<S: SSOT + ?Sized>(ssot: &S) -> String {
    let mut out = String::from("{");
    let _ = write!(out, "\"name\": \"{}\"", ssot.name());
    if !ssot.section().is_empty() {
        let _ = write!(out, ", \"section\": \"{}\"", ssot.section());
    }
    for (k, v) in ssot.rows() {
        let _ = write!(out, ", \"{k}\": \"{v}\"");
    }
    out.push('}');
    out
}

fn render_template_placeholder<S: SSOT + ?Sized>(ssot: &S) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "name = {{{{ name }}}}");
    if !ssot.section().is_empty() {
        let _ = writeln!(out, "section = {{{{ section }}}}");
    }
    for (k, _) in ssot.rows() {
        let _ = writeln!(out, "{k} = {{{{ {k} }}}}");
    }
    out
}

// ---------------------------------------------------------------------------
// Stub implementation — an environment name + region pair.
// Kept deliberately tiny so the doc tests and inline tests
// stay readable; real callers bring their own structs.
// ---------------------------------------------------------------------------

/// Example `SSOT` impl: a deployment environment identifier.
/// The struct is the SSOT; the renderers are the projections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    /// Short environment name (e.g. `"prod"`, `"staging"`).
    pub env: String,
    /// Cloud region (e.g. `"us-east-1"`).
    pub region: String,
    /// Optional human-friendly label.
    pub label: Option<String>,
}

impl Environment {
    /// Build an `Environment` from a short name. `region`
    /// defaults to `"us-east-1"`, `label` is `None`.
    pub fn ssot(env: impl Into<String>) -> Self {
        Self {
            env: env.into(),
            region: "us-east-1".to_string(),
            label: None,
        }
    }
}

impl SSOT for Environment {
    fn name(&self) -> &str {
        // The label, if set, is a friendlier name.
        // Otherwise we return a static fallback — we
        // can't return `&self.env`'s content for a
        // borrowed string without tying it to `self`'s
        // lifetime, so we use a literal.
        self.label.as_deref().unwrap_or("environment")
    }

    fn section(&self) -> &str {
        "Deployment"
    }

    fn rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("env", self.env.clone()),
            ("region", self.region.clone()),
            ("label", self.label.clone().unwrap_or_default()),
        ]
    }
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    /// Smoke test: a stub `SSOT` impl renders to the three
    /// formats without panicking and produces the expected
    /// structural shape.
    #[test]
    fn environment_renders_to_three_formats() {
        let env = Environment::ssot("prod");
        let md = env.render(Render::Markdown);
        assert!(md.contains("## Deployment"), "markdown: {md}");
        assert!(md.contains("- **env:** `prod`"), "markdown: {md}");

        let json = env.render(Render::Json);
        assert!(json.starts_with('{') && json.ends_with('}'), "json: {json}");
        assert!(json.contains("\"env\": \"prod\""), "json: {json}");

        let tpl = env.render(Render::TemplatePlaceholder);
        assert!(tpl.contains("env = {{ env }}"), "template: {tpl}");
    }
}
