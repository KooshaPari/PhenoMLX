//! Sample GSD document for tests/fixtures.

# Project Foo

## Task 1: Bootstrap repo
Initialize git, add README, add CI skeleton.

- [x] git init
- [ ] README with one-liner
- [ ] CI: cargo check on push

## Task 2: Add CLI entrypoint
Single-file Go binary, subcommand router, `--help` works.

- [x] binary builds
- [x] --help works
- [ ] --version works
- [ ] subcommands wired

## Task 3: Persist state
Use SQLite WAL + flock for multi-agent concurrency.

- [x] schema migrated
- [x] init works on existing DB
