# pheno-flags

Typed feature-flag resolution. A `Resolver` reads values in a
fixed order:

1. **envvar** (the only path that touches `std::env`),
2. **.env-style file** the caller loaded at build time,
3. **caller-supplied default**.

All public methods are typed: `bool(...)`, `i64(...)`,
`string(...)`. A failed parse is a `FlagError::Parse` that
carries the source (`"env"`, `"file"`, or `"default"`) and
the raw value — never a silent fall-through.

## Usage

```rust
use pheno_flags::Resolver;

let r = Resolver::empty()
    .env("DARK_MODE")
    .file("DARK_MODE=1\nMAX_CONN=64\n")
    .default_bool("DARK_MODE", false)
    .default_i64("MAX_CONN", 8);

let dark: bool  = r.bool("DARK_MODE").unwrap();
let maxc: i64   = r.i64("MAX_CONN").unwrap();
```

## Why typed?

Stringly-typed feature flags are an endless source of
defects: a typo, a missing parse step, a default that's a
`String` where a `bool` was wanted. The three typed
accessors make those defects either compile errors (wrong
default type for a typed lookup) or runtime `FlagError`s
(env value `not-a-number` for an `i64` lookup).

## Tests

`cargo test --offline -p pheno-flags` runs:

- 1 inline smoke test
- 6 integration tests (env-wins, file-wins, default-fallback,
  parse-error, bool grammar, file parser)
- 2 doc tests
