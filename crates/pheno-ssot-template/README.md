# pheno-ssot-template

A typed **Single-Source-Of-Truth (SSOT) trait** + struct-and-render pattern.

The pattern: define a struct once, then have it render itself in any
of the stable formats you need (Markdown, JSON, template placeholder)
without restating the same fields in three different strings.

## Why

A typical codebase restates the same value — a service name, an
environment, a feature flag — in:

- the README,
- a CI matrix YAML,
- a Helm values file,
- a Slack alert template.

When the value changes, three of those four will drift. The
`SSOT` trait centralises the struct, and the three renderers
become the only projections to keep up to date.

## Usage

```rust
use pheno_ssot_template::{SSOT, Render, render};

struct ServiceConfig;
impl SSOT for ServiceConfig {
    fn name(&self) -> &str { "billing-api" }
    fn section(&self) -> &str { "Services" }
    fn rows(&self) -> Vec<(&'static str, String)> {
        vec![("replicas", "3".to_string()),
             ("image", "ghcr.io/acme/billing:v1.2.0".to_string())]
    }
}

let cfg = ServiceConfig;
let md  = render(&cfg, Render::Markdown);
let js  = render(&cfg, Render::Json);
let tpl = render(&cfg, Render::TemplatePlaceholder);
```

## Renderers

| Format | Output shape |
|--------|--------------|
| `Render::Markdown` | `## {section}` + `**name:**` + `- **key:** \`value\`` rows |
| `Render::Json` | `{ "name": "...", "section": "...", "key": "value" }` |
| `Render::TemplatePlaceholder` | `key = {{ key }}` (handlebars/minijinja friendly) |

## Tests

`cargo test --offline -p pheno-ssot-template` runs:

- 1 inline smoke test
- 5 integration tests (markdown shape, json shape, template
  shape, `Render::as_str`, `Environment` stub)
- 1 doc test
