# agileplus-application

Hexagonal application and use-case layer with no framework or storage dependencies.

## Public API Index

- Modules: `dto`, `error`, `events`, `use_cases`.
- Use cases are structs wired with explicit port dependencies.
- Errors are surfaced through the application error module.

## Validation

```bash
cargo test -p agileplus-application
```

