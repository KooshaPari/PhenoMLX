# agileplus-sqlite

SQLite persistence adapter for AgilePlus storage ports.

## Public API Index

- `SqliteStorageAdapter`: SQLite-backed storage implementation.
- Modules: `migrations`, `rebuild`, `repository`, `seed`.
- Uses WAL mode and foreign keys per crate docs.

## Validation

```bash
cargo test -p agileplus-sqlite
```

