//! # pheno-vibecoding-guard — AST-based heuristic linter
//!
//! Five heuristics targeted at the patterns most often
//! produced by AI-coding assistants (long, deeply-nested
//! functions, unused parameters, excessive `.unwrap()`,
//! and placeholder comments). The linter is built on
//! [`syn`](https://docs.rs/syn) so it sees a full Rust
//! AST, not a line-by-line token stream.
//!
//! ## Heuristics
//!
//! | # | Name                | Default threshold | What it catches |
//! |---|---------------------|-------------------|-----------------|
//! | 1 | `LongFunctionBody`  | > 50 lines        | Vibecoded "wall of code" |
//! | 2 | `DeepNesting`       | depth > 4         | `if`/`for`/`while`/`match` stacks |
//! | 3 | `UnusedParameter`   | a `fn` param never used in the body | `_x` is still flagged as a smell |
//! | 4 | `ExcessiveUnwrap`   | > 5 `.unwrap()` calls per function | Cop-out error handling |
//! | 5 | `TodoComment`       | `TODO` / `FIXME` / `HACK` strings | Placeholder code shipped as final |
//!
//! The thresholds are exposed via [`LintConfig`] and can
//! be tightened (CI) or relaxed (ad-hoc review) without
//! recompiling.
//!
//! ## Quick start
//!
//! ```
//! use pheno_vibecoding_guard::{lint_source, LintConfig};
//!
//! let src = r#"
//!     fn short() -> i32 {
//!         let x = 1;
//!         x + 1
//!     }
//! "#;
//! let findings = lint_source(src, &LintConfig::default());
//! assert!(findings.is_empty(), "short, well-formed fn must be clean");
//! ```

use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{ItemFn, Stmt};

/// A single linter finding.
///
/// `kind` is a stable string identifier (one of the five
/// `Kind` constants below) so callers can group / count /
/// suppress by name. `line` and `column` are 1-based.
#[derive(Debug, Clone)]
pub struct Finding {
    /// Stable string identifier of the heuristic that fired.
    /// One of `"LongFunctionBody"`, `"DeepNesting"`,
    /// `"UnusedParameter"`, `"ExcessiveUnwrap"`,
    /// `"TodoComment"`.
    pub kind: String,
    /// Human-readable explanation, safe to log.
    pub message: String,
    /// 1-based line number of the offending AST node.
    pub line: u32,
    /// 1-based column number of the offending AST node.
    pub column: u32,
}

impl Finding {
    fn at(kind: &str, message: impl Into<String>, span: proc_macro2::Span) -> Self {
        let start = span.start();
        Self {
            kind: kind.to_string(),
            message: message.into(),
            line: u32::try_from(start.line).unwrap_or(u32::MAX),
            column: u32::try_from(start.column).unwrap_or(u32::MAX),
        }
    }
}

/// Tunable thresholds for the linter.
///
/// Construct with [`LintConfig::default`] and override
/// only the fields the caller cares about.
#[derive(Debug, Clone)]
pub struct LintConfig {
    /// `LongFunctionBody` fires when a function body's
    /// source line count exceeds this threshold.
    /// Default: 50.
    pub max_function_lines: u32,
    /// `DeepNesting` fires when the maximum depth of
    /// `if`/`for`/`while`/`match` blocks inside a
    /// function exceeds this threshold. Default: 4.
    pub max_nesting_depth: u32,
    /// `ExcessiveUnwrap` fires when a function contains
    /// more than this many `.unwrap()` method calls.
    /// Default: 5.
    pub max_unwraps: u32,
    /// Whether to scan for `TODO` / `FIXME` / `HACK`
    /// strings in the source text. Default: `true`.
    pub check_todo_comments: bool,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            max_function_lines: 50,
            max_nesting_depth: 4,
            max_unwraps: 5,
            check_todo_comments: true,
        }
    }
}

/// Run the full linter over `source` and return every
/// finding, in the order they were discovered.
///
/// `source` is a full Rust source unit (one or more
/// `fn`s, `mod`s, `struct`s, etc.); it is parsed via
/// [`syn::parse_file`]. A parse error is reported as a
/// `Finding` with `kind = "ParseError"` and the error
/// message as `message`.
pub fn lint_source(source: &str, config: &LintConfig) -> Vec<Finding> {
    let mut findings = Vec::new();

    // Heuristic 5: TODO/FIXME/HACK comment scan. This is
    // a textual check on the raw source — it runs before
    // the AST walk so the linter still flags comments
    // even if the source fails to parse.
    if config.check_todo_comments {
        scan_todo_comments(source, &mut findings);
    }

    // Heuristics 1-4: AST walk. A parse error is
    // surfaced as a single finding; we do not silently
    // return an empty vector.
    let Ok(file) = syn::parse_file(source) else {
        findings.push(Finding {
            kind: "ParseError".to_string(),
            message: "source failed to parse as a Rust file".to_string(),
            line: 1,
            column: 1,
        });
        return findings;
    };

    let mut visitor = LintVisitor {
        config,
        findings: std::mem::take(&mut findings),
    };
    visitor.visit_file(&file);
    visitor.findings
}

fn scan_todo_comments(source: &str, findings: &mut Vec<Finding>) {
    for (idx, line) in source.lines().enumerate() {
        for needle in ["TODO", "FIXME", "HACK"] {
            if line.contains(needle) {
                findings.push(Finding {
                    kind: "TodoComment".to_string(),
                    message: format!("`{needle}` placeholder comment left in source"),
                    line: u32::try_from(idx + 1).unwrap_or(u32::MAX),
                    column: 1,
                });
                break; // one finding per line
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AST visitor
// ---------------------------------------------------------------------------

struct LintVisitor<'c> {
    config: &'c LintConfig,
    findings: Vec<Finding>,
}

impl<'c> LintVisitor<'c> {
    /// Heuristic 1: a function body whose source span
    /// covers more than `max_function_lines` lines.
    fn check_long_function_body(&mut self, item_fn: &ItemFn) {
        let block_span = item_fn.block.span();
        let start = block_span.start();
        let end = block_span.end();
        let line_count = u32::try_from(end.line.saturating_sub(start.line) + 1).unwrap_or(u32::MAX);
        if line_count > self.config.max_function_lines {
            self.findings.push(Finding::at(
                "LongFunctionBody",
                format!(
                    "function `{}` body is {line_count} lines (max {})",
                    item_fn.sig.ident, self.config.max_function_lines,
                ),
                item_fn.sig.ident.span(),
            ));
        }
    }

    /// Heuristic 2: a function body whose maximum
    /// `if`/`for`/`while`/`match` nesting depth exceeds
    /// `max_nesting_depth`.
    fn check_deep_nesting(&mut self, item_fn: &ItemFn) {
        let mut counter = DepthCounter { depth: 0, max: 0 };
        counter.visit_block(&item_fn.block);
        if counter.max > self.config.max_nesting_depth {
            self.findings.push(Finding::at(
                "DeepNesting",
                format!(
                    "function `{}` reaches nesting depth {} (max {})",
                    item_fn.sig.ident, counter.max, self.config.max_nesting_depth,
                ),
                item_fn.sig.ident.span(),
            ));
        }
    }

    /// Heuristic 3: a function parameter that is never
    /// referenced in the function body. `_x` is still
    /// counted as a smell — the heuristic is
    /// intentionally conservative, because a vibecoder
    /// often writes `fn foo(_x: i32)` to silence the
    /// unused-param warning without actually using the
    /// value.
    fn check_unused_parameters(&mut self, item_fn: &ItemFn) {
        for input in &item_fn.sig.inputs {
            let pat = match input {
                syn::FnArg::Typed(pat_type) => &pat_type.pat,
                syn::FnArg::Receiver(_) => continue,
            };
            // Pull the bare identifier out of the
            // pattern. For the common `name: T` shape,
            // the pattern is `Pat::Ident`. For
            // destructuring patterns (`(a, b): (i32,
            // i32)`), we conservatively skip — that path
            // is not a vibecoding smell, the parameter is
            // a tuple destructuring.
            let ident = match &**pat {
                syn::Pat::Ident(pi) => pi.ident.to_string(),
                _ => continue,
            };
            if ident.starts_with('_') {
                // Underscore-prefixed is the conventional
                // way to silence "unused" warnings. It's
                // still a vibecoding smell: the function
                // declared a parameter it never uses.
                self.findings.push(Finding::at(
                    "UnusedParameter",
                    format!(
                        "function `{}` has underscore-prefixed parameter `{ident}`",
                        item_fn.sig.ident,
                    ),
                    pat.span(),
                ));
                continue;
            }
            // Walk the function body and look for any
            // reference to the identifier.
            let mut finder = IdentFinder {
                target: &ident,
                found: false,
            };
            finder.visit_block(&item_fn.block);
            if !finder.found {
                self.findings.push(Finding::at(
                    "UnusedParameter",
                    format!(
                        "function `{}` parameter `{ident}` is never used",
                        item_fn.sig.ident,
                    ),
                    pat.span(),
                ));
            }
        }
    }

    /// Heuristic 4: a function body that calls
    /// `.unwrap()` more than `max_unwraps` times. The
    /// walk is post-order; we count `MethodCall {
    /// method: "unwrap", .. }` nodes anywhere in the
    /// function body.
    fn check_excessive_unwrap(&mut self, item_fn: &ItemFn) {
        let mut counter = UnwrapCounter { count: 0 };
        counter.visit_block(&item_fn.block);
        if counter.count > self.config.max_unwraps {
            self.findings.push(Finding::at(
                "ExcessiveUnwrap",
                format!(
                    "function `{}` has {} `.unwrap()` calls (max {})",
                    item_fn.sig.ident, counter.count, self.config.max_unwraps,
                ),
                item_fn.sig.ident.span(),
            ));
        }
    }
}

impl<'c, 'ast> Visit<'ast> for LintVisitor<'c> {
    fn visit_item_fn(&mut self, item_fn: &'ast ItemFn) {
        self.check_long_function_body(item_fn);
        self.check_deep_nesting(item_fn);
        self.check_unused_parameters(item_fn);
        self.check_excessive_unwrap(item_fn);
        // Recurse into nested items (fn-within-fn, etc.)
        // so heuristics fire on the inner functions too.
        visit::visit_item_fn(self, item_fn);
    }

    fn visit_stmt(&mut self, node: &'ast Stmt) {
        // Walk into nested items inside `fn body() { fn
        // inner() {} }`. The default visitor handles
        // this for `Item::Fn`, but `Stmt::Item` needs an
        // explicit delegation.
        visit::visit_stmt(self, node);
    }
}

// ---------------------------------------------------------------------------
// Helper visitors
// ---------------------------------------------------------------------------

/// Computes the maximum `if`/`for`/`while`/`match` depth
/// inside a block.
struct DepthCounter {
    depth: u32,
    max: u32,
}

impl<'ast> Visit<'ast> for DepthCounter {
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.depth += 1;
        self.max = self.max.max(self.depth);
        visit::visit_expr_if(self, node);
        self.depth -= 1;
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.depth += 1;
        self.max = self.max.max(self.depth);
        visit::visit_expr_for_loop(self, node);
        self.depth -= 1;
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.depth += 1;
        self.max = self.max.max(self.depth);
        visit::visit_expr_while(self, node);
        self.depth -= 1;
    }

    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        self.depth += 1;
        self.max = self.max.max(self.depth);
        visit::visit_expr_match(self, node);
        self.depth -= 1;
    }
}

/// Counts `.unwrap()` method calls anywhere inside a
/// block.
struct UnwrapCounter {
    count: u32,
}

impl<'ast> Visit<'ast> for UnwrapCounter {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "unwrap" {
            self.count += 1;
        }
        visit::visit_expr_method_call(self, node);
    }
}

/// Looks for a bare identifier reference inside a block.
/// Stops at the first hit. Used for the unused-parameter
/// check.
struct IdentFinder<'a> {
    target: &'a str,
    found: bool,
}

impl<'a, 'ast> Visit<'ast> for IdentFinder<'a> {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if self.found {
            return;
        }
        if let Some(ident) = node.path.get_ident() {
            if ident == self.target {
                self.found = true;
            }
        }
        visit::visit_expr_path(self, node);
    }
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    /// Inline smoke test: a clean function yields zero
    /// findings. The integration test file covers each
    /// heuristic in isolation.
    #[test]
    fn clean_function_yields_no_findings_inline() {
        let src = r#"
            fn add(a: i32, b: i32) -> i32 {
                a + b
            }
        "#;
        let findings = lint_source(src, &LintConfig::default());
        assert!(
            findings.is_empty(),
            "a trivial `add` function must not trip any heuristic, got: {findings:?}",
        );
    }
}
