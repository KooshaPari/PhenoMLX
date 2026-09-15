# Contributing

Thanks for your interest in contributing!

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/REPO.git`
3. Create a feature branch: `git checkout -b feat/your-feature`

## Development

```bash
# Build
cargo build --workspace

# Test
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings

# Format
cargo fmt
```

## Code Style

- Follow `cargo fmt` defaults (rustfmt.toml is committed)
- All lints pass clippy with `-D warnings`
- No new lint suppressions without inline justification (`// allow(...): reason`)
- Add tests for new behavior
- Update CHANGELOG.md for user-facing changes

## Pull Requests

- Use the PR template (`.github/PULL_REQUEST_TEMPLATE.md`)
- Reference issues (`Closes #N`, `Fixes #N`)
- Keep PRs focused; one concern per PR
- Rebase on main before requesting review
- All checks must pass (cargo-deny, CodeQL Rust, pre-commit hooks)

## Reporting Bugs

Use the issue template at `.github/ISSUE_TEMPLATE/bug.md`.
For security issues, see `SECURITY.md`.

## License

By contributing, you agree your contributions will be licensed under the project's existing license.
