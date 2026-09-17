# pheno-ci-templates

Reusable GitHub Actions workflow templates. The deliverable
is YAML, not Rust — the crate exists so the workflow files
have a versioned, reviewable home and a Rust-side test that
locks their shape against drift.

## Layout

| File                       | What it adds on top of `ci-base` |
|----------------------------|----------------------------------|
| `.github/workflows/ci-base.yml`   | checkout, concurrency, `workflow_call` |
| `.github/workflows/ci-rust.yml`   | `cargo fmt` / `clippy` / `test` |
| `.github/workflows/ci-node.yml`   | `pnpm install` / `tsc` / `vitest` |
| `.github/workflows/ci-python.yml` | `uv sync` / `ruff` / `pytest` |

## Usage from a consuming repo

```yaml
# .github/workflows/ci.yml
name: ci
on: [push, pull_request]
jobs:
  rust:
    uses: agileplus-ai/pheno-ci-templates/.github/workflows/ci-rust.yml@v0.1
  node:
    uses: agileplus-ai/pheno-ci-templates/.github/workflows/ci-node.yml@v0.1
```

The consumer picks the language jobs they want and pins
the tag. A bump to the cache key, the checkout depth, or
the runner image happens in this repo, not in 14 forks.

## Tests

`cargo test --offline -p pheno-ci-templates` runs:

- 1 inline smoke test (path constants well-formed)
- 3 integration tests (all files exist, every workflow
  declares `workflow_call`, every workflow has a `name:`)
