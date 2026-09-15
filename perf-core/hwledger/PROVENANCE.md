# PROVENANCE: hwLedger

## Source Repository

- **Repository**: [KooshaPari/zz-merge-unk-hwLedger](https://github.com/KooshaPari/zz-merge-unk-hwLedger)
- **Absorbed**: 2026-09-14
- **Branch**: `absorb-hwledger` in KooshaPari/PhenoMLX

## What Was Migrated

Source code from hwLedger, a 526MB Rust hardware ledger repository.

### Directories
- `crates/` - Rust crates (hwledger-bench-cockpit, hwledger-cli)
- `apps/` - Applications (bench-matrix, hwledger-app, model-explorer)
- `sidecars/` - Sidecar services (bench-cockpit, omlx-fork)
- `data/` - Benchmarks, fixtures, and run data
- `docs/` - Documentation
- `scripts/` - Build and utility scripts
- `tests/` - Test suite

### Root Files
- `Cargo.toml`, `Cargo.lock` - Rust workspace configuration
- `clippy.toml` - Clippy linting config
- `trunk.yaml` - Trunk CI config
- `AGENTS.md`, `CLAUDE.md` - Agent configuration
- `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md` - Project documentation
- `LICENSE`, `LICENSE-MIT`, `SECURITY.md`, `CODE_OF_CONDUCT.md` - Legal/licensing
- `CITATION.cff`, `llms.txt`, `RICH_MEDIA.md`, `WORKLOG.md` - Metadata
- `audit_scorecard.json` - Audit data
- `tsconfig.json`, `vitest.config.ts` - TypeScript/test config (if applicable)

## What Was Excluded

- `.git/` - Git history (source repo preserved independently)
- `.circleci/`, `.github/` - CI configuration (PhenoMLX has its own)
- `.trunk/`, `.claude/` - Tool-specific configs (PhenoMLX has its own)
- `.editorconfig`, `.pre-commit-config.yaml`, `.gitattributes`, `.gitignore` - Repo-level configs
- `.mergify.yml`, `renovate.json`, `deny.toml`, `CODEOWNERS` - Governance configs
- `FUNDING.yml` - Funding metadata
- Binary assets larger than 1MB (if any)

## Integration Notes

- hwLedger code placed in `perf-core/hwledger/` as a sub-workspace
- May require Cargo workspace member registration in `perf-core/Cargo.toml`
- Integration testing recommended before merging to main
