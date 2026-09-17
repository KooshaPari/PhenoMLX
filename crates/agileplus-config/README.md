# agileplus-config

Shared config-builder macro support for AgilePlus crates.

## Public API Index

- `config_builder!`: creates config structs with defaults and `with_<field>` setters.
- Re-exports `paste` for macro expansion.
- Field tags include `str`, `opt_str`, and `val`.

## Validation

```bash
cargo test -p agileplus-config
```

