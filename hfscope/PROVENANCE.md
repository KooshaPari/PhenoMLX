# Provenance: hfscope

**Source Repository**: [<REDACTED>/zz-merge-unk-hfscope](https://github.com/KooshaPari/zz-merge-unk-hfscope)
**Absorption Date**: 2026-09-15
**Absorbed Into**: <REDACTED>/PhenoMLX (`hfscope/`)

## What is hfscope?

A Go-based HuggingFace scope tool (~10MB source). It provides:
- HuggingFace API client (`internal/hfapi/`)
- TTL cache layer (`internal/cache/`)
- Configuration management (`internal/config/`)
- Web server with templ-based rendering (`internal/server/`)
- Views/data layer (`internal/views/`)
- Chrome extension for HuggingFace browsing (`hfscope-extension/`)
- CLI tools (`cmd/hfscope/`, `cmd/hfscope-token/`)

## Source Info

- **Go module**: `github.com/KooshaPari/hfscope`
- **Go version**: 1.26.5
- **License**: See `hfscope/LICENSE`
- **Last commit at absorption**: See source repo commit history

## Files Absorbed

Full source tree copied to `hfscope/`, excluding:
- `.git/` history
- `STATUS.md`, `CHANGELOG.md` (repo metadata, not source)
- `.mergify.yml`, `renovate.json` (CI/CD config specific to source repo)

## License

The absorbed source retains its original license as found in `hfscope/LICENSE`.
