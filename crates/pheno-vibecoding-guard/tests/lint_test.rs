//! Integration tests for the vibecoding-guard linter.
//!
//! Each heuristic has its own test so a future regression
//! points at exactly one rule. The clean-function test
//! lives in `lib.rs` as an inline smoke test.

use pheno_vibecoding_guard::{lint_source, LintConfig};

/// Helper: build a `LintConfig` with the unwrap threshold
/// set to a specific value. Keeps the test bodies focused
/// on the source string, not the boilerplate.
fn cfg(max_unwraps: u32) -> LintConfig {
    LintConfig {
        max_unwraps,
        ..LintConfig::default()
    }
}

#[test]
fn long_function_body_fires() {
    // Build a function body that is unambiguously longer
    // than the 50-line default. The body has a single
    // `let _x = ...;` line per source line; the count is
    // exact and easy to reason about.
    let mut body = String::from("fn long() {\n");
    for i in 0..60 {
        body.push_str(&format!("    let _x{i} = {i};\n"));
    }
    body.push_str("}\n");
    let findings = lint_source(&body, &LintConfig::default());
    assert!(
        findings.iter().any(|f| f.kind == "LongFunctionBody"),
        "expected LongFunctionBody, got {findings:?}",
    );
}

#[test]
fn deep_nesting_fires() {
    // 5 levels of `if`, default max is 4.
    let src = r#"
        fn deep(x: i32) -> i32 {
            if x > 0 {
                if x > 1 {
                    if x > 2 {
                        if x > 3 {
                            if x > 4 {
                                return 5;
                            }
                            return 4;
                        }
                        return 3;
                    }
                    return 2;
                }
                return 1;
            }
            0
        }
    "#;
    let findings = lint_source(src, &LintConfig::default());
    assert!(
        findings.iter().any(|f| f.kind == "DeepNesting"),
        "expected DeepNesting, got {findings:?}",
    );
}

#[test]
fn unused_parameter_fires_for_unused_and_underscore() {
    let src = r#"
        fn uses_none(a: i32, b: i32) -> i32 { 0 }
        fn uses_underscore(_x: i32) -> i32 { 0 }
    "#;
    let findings = lint_source(src, &LintConfig::default());
    let unused: Vec<_> = findings
        .iter()
        .filter(|f| f.kind == "UnusedParameter")
        .collect();
    // `uses_none` has two unused parameters (a, b) and
    // `uses_underscore` has one. Three findings total.
    assert_eq!(
        unused.len(),
        3,
        "expected 3 UnusedParameter findings, got {unused:?}",
    );
    let messages: Vec<_> = unused.iter().map(|f| f.message.as_str()).collect();
    assert!(
        messages.iter().any(|m| m.contains("`a`")),
        "expected a finding for `a`, got {messages:?}",
    );
    assert!(
        messages.iter().any(|m| m.contains("`b`")),
        "expected a finding for `b`, got {messages:?}",
    );
    assert!(
        messages.iter().any(|m| m.contains("underscore-prefixed")),
        "expected an underscore-prefixed finding, got {messages:?}",
    );
}

#[test]
fn excessive_unwrap_fires_above_threshold() {
    // 6 unwraps, default max is 5.
    let src = r#"
        fn messy(v: Vec<i32>) -> i32 {
            v[0].unwrap()
             + v[1].unwrap()
             + v[2].unwrap()
             + v[3].unwrap()
             + v[4].unwrap()
             + v[5].unwrap()
        }
    "#;
    // The 6 calls use the literal `.unwrap()` method —
    // we need a value to call it on. `v[i]` is a `i32`
    // which doesn't have `.unwrap()` in real Rust, but
    // `syn` is happy to parse it and our linter doesn't
    // type-check. The point is the method-call shape.
    let findings = lint_source(src, &cfg(5));
    assert!(
        findings.iter().any(|f| f.kind == "ExcessiveUnwrap"),
        "expected ExcessiveUnwrap, got {findings:?}",
    );

    // The same source with a higher threshold must NOT
    // fire — proves the threshold is honoured.
    let findings = lint_source(src, &cfg(10));
    assert!(
        findings.iter().all(|f| f.kind != "ExcessiveUnwrap"),
        "ExcessiveUnwrap fired below the threshold, got {findings:?}",
    );
}

#[test]
fn todo_comment_fires() {
    let src = r#"
        fn incomplete() -> i32 {
            // TODO: implement me
            0
        }
    "#;
    let findings = lint_source(src, &LintConfig::default());
    assert!(
        findings.iter().any(|f| f.kind == "TodoComment"),
        "expected TodoComment, got {findings:?}",
    );
}
