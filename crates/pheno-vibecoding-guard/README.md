# pheno-vibecoding-guard

An **AST-based** heuristic linter for AI-generated (a.k.a.
"vibecoded") Rust code. Built on top of
[`syn`](https://docs.rs/syn), so it sees the full Rust AST
rather than a token stream.

## Heuristics

| # | Name                | Default threshold | What it catches |
|---|---------------------|-------------------|-----------------|
| 1 | `LongFunctionBody`  | > 50 lines        | Vibecoded "wall of code" |
| 2 | `DeepNesting`       | depth > 4         | `if`/`for`/`while`/`match` stacks |
| 3 | `UnusedParameter`   | a `fn` param never used in the body | `_x` is still flagged as a smell |
| 4 | `ExcessiveUnwrap`   | > 5 `.unwrap()` calls per function | Cop-out error handling |
| 5 | `TodoComment`       | `TODO` / `FIXME` / `HACK` strings | Placeholder code shipped as final |

All thresholds are exposed on `LintConfig` and can be
tightened in CI or relaxed for ad-hoc review.

## Usage

```rust
use pheno_vibecoding_guard::{lint_source, LintConfig};

let cfg = LintConfig {
    max_function_lines: 30,
    max_nesting_depth: 3,
    max_unwraps: 2,
    check_todo_comments: true,
};

let findings = lint_source(source, &cfg);
for f in &findings {
    eprintln!("{}:{} {} — {}", f.line, f.column, f.kind, f.message);
}
```

## Why AST-based?

A regex linter sees `if if if if` and counts four
indents. A `syn`-based linter sees four `ExprIf` nodes
nesting each other — and crucially, it can tell the
difference between real control flow and a string that
contains the word `if`. The same applies to `.unwrap()`:
the linter counts `ExprMethodCall` nodes whose `method`
field is `"unwrap"`, not substring matches.

## Tests

`cargo test --offline -p pheno-vibecoding-guard` runs:

- 1 inline smoke test (clean function yields no findings)
- 5 integration tests (one per heuristic, isolated)
- 1 doc test
