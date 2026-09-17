# agileplus-cli

Top-level `agileplus` CLI crate and command wiring.

## Public API Index

- Context types: `CommandContext`, `StorageOnlyContext`, `CommandTelemetry`, `OutputFormat`.
- Formatting trait: `TableFormattable`.
- Command modules implement the CLI subcommands used by the binary.

## Validation

```bash
cargo test -p agileplus-cli
```

