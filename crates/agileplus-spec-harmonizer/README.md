# agileplus-spec-harmonizer

> Migrated from `KooshaPari/agileplus-spec-harmonizer` v0.1.0 on 2026-06-18.
> See `docs/adr/2026-06-18/ADR-NNN-spec-harmonizer-absorption.md` for the
> absorption decision matrix.

Harmonize work packages from **GSD**, **OpenSpec**, **BMAD-Method**, and **Spec-Kitty**
into a single normalized shape, then emit as NDJSON (TRAC-aligned) or a Markdown index.

Designed to be the **ingest** half of the AgilePlus SDD pipeline:

```
gsd/openspec/bmad/kitty specs  →  harmonize  →  WorkPackage[]  →  agileplus-subcmds  →  Tracera
```

> **Status:** This crate is the *parser*. The downstream consumer
> `agileplus-subcmds` is currently a **stub** (see `crates/agileplus-subcmds/src/lib.rs`).
> Once the `agileplus tracera list` bridge lands, this crate will be wired
> in as the head of the SDD pipeline.

## What it does

Each spec format has its own quirks:

| Format       | Heading                              | Acceptance block       | Source anchor  |
|--------------|--------------------------------------|------------------------|----------------|
| GSD          | `## Task N: <title>`                 | `- [ ]` / `- [x]`      | `task-N`       |
| OpenSpec     | `## Spec <id> — <title>`             | `## Acceptance`        | `<id>`         |
| BMAD-Method  | `## Story <id>: <title>`             | `## Criteria`          | `<id>`         |
| Spec-Kitty   | `## Spec <id> - <title>` (hyphen)    | `## Acceptance`        | `<id>`         |

This crate parses all four into one `WorkPackage`:

```rust
pub struct WorkPackage {
    pub id: String,             // "gsd-1", "openspec-ABC-1", "bmad-S1", "kitty-K-1"
    pub title: String,
    pub description: String,
    pub acceptance: Vec<AcceptanceCriterion>,
    pub source_format: String,  // "gsd" | "openspec" | "bmad" | "kitty"
    pub source_anchor: String,  // original spec anchor (e.g. "1", "ABC-1")
}
```

The `source_format` field is preserved (not discarded) so the crate can round-trip back
to the original format if a future `agileplus harmonize --emit json` is added.

## Quick start

```rust
use agileplus_spec_harmonizer::{parsers::Parser, parsers::gsd::GsdParser};

let text = std::fs::read_to_string("spec.md").unwrap();
let pkgs = GsdParser.parse(&text).unwrap();
println!("parsed {} GSD tasks", pkgs.len());
```

Or use the dispatcher:

```rust
use agileplus_spec_harmonizer::{parse, Format};

let text = std::fs::read_to_string("spec.md")?;
let packages = parse(&text, Format::OpenSpec)?;
```

## Modules

| Module           | Purpose                                                |
|------------------|--------------------------------------------------------|
| `parsers`        | 4 format-specific parsers behind a `Parser` trait      |
| `normalize`      | `slug()`, `stable_hash()` (FNV-1a 64-bit), `merge()`   |
| `emit`           | `emit_ndjson()`, `emit_markdown()`                     |

## Tests

12 unit + integration tests, all passing:

```bash
cargo test -p agileplus-spec-harmonizer
```

The integration test (`tests/integration.rs`) parses
`fixtures/gsd_sample.md` end-to-end and asserts shape, count, and
acceptance-criteria granularity.

## Design constraints

- **Pure Rust, no async, no cgo.** The `Cargo.toml` has zero `tokio` /
  `async-std` dependencies. Parsing is fully synchronous.
- **No external LLM calls.** This is a deterministic markdown parser,
  not an AI model. Slop issues are caught by the integration fixture.
- **No mutation of source.** The crate reads text and produces owned
  `WorkPackage` values. The original spec is never touched.

## License

MIT OR Apache-2.0 (inherited from `AgilePlus` workspace).
