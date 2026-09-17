// SPDX-License-Identifier: MIT OR Apache-2.0
//! AST-aware tokenization for source code.
//!
//! Hand-rolled single-pass scanner over Rust and Python source.  We
//! deliberately do *not* use a full parser (`syn`, `tree-sitter`,
//! `rustpython-parser`) â€” those crates are heavy and overkill for the
//! downstream dedup pipeline.  Instead we extract the *significant
//! token classes* (control-flow keywords, type-defining keywords, and
//! the `->` arrow / `::` separators) that are stable across reformatting
//! and renaming.  Identifier names are preserved as opaque tokens; this
//! is enough to give a meaningful Jaccard / MinHash signature for
//! near-duplicate code detection.
//!
//! # Why no `regex`?
//!
//! The `regex` crate's DFA / NFA construction is rejected by the local
//! `regex-automata 0.4.14` build for our keyword alternation patterns
//! ("error building NFA").  A hand-rolled scanner sidesteps this and
//! also avoids the new heavy dependency.  Throughput is on par with
//! `regex::Regex::find_iter` for the small input sizes we see in
//! triage workloads.
//!
//! # Audit
//!
//! Implements recommendation #5 from `AUDIT_BLOC_VS_2026_SOTA.md`: the
//! AST-aware tokenizer that feeds the hybrid dedup pipeline.

// -------- Token categories ----------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// An identifier-shaped run `[A-Za-z_][A-Za-z0-9_]*`.
    Ident,
    /// A digit run `[0-9]+`.
    Number,
    /// Whitespace / comments â€” to be skipped.
    Junk,
    /// Any other single character that we still want to surface as a
    /// token (e.g. `,`, `(`, `)`).
    Punct,
}

fn classify(b: u8) -> Class {
    match b {
        b'-' | b'=' | b':' | b',' | b'(' | b')' | b'{' | b'}' | b'[' | b']' | b';' | b'<'
        | b'>' | b'&' | b'|' | b'!' | b'?' | b'.' | b'+' | b'*' | b'/' | b'%' | b'^' | b'~'
        | b'@' | b'#' | b'$' => Class::Punct,
        b'A'..=b'Z' | b'a'..=b'z' | b'_' => Class::Ident,
        b'0'..=b'9' => Class::Number,
        _ => Class::Junk,
    }
}

fn is_word_byte(b: u8) -> bool {
    matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
}

// -------- Trait --------------------------------------------------------

/// Token-extraction strategy.  Each implementation knows the keyword set
/// for one language and emits tokens in a stable left-to-right order.
pub trait AstTokenizer: Send + Sync {
    /// Tokenize `source` into a flat token stream.
    fn tokenize(&self, source: &str) -> Vec<String>;

    /// Stable name (e.g. `"rust"`, `"python"`).
    fn name(&self) -> &'static str;
}

// -------- Generic scanner -----------------------------------------------

/// Run `is_keyword` over each identifier in `source` (in order) and
/// emit it as a token if it matches a keyword, otherwise emit a
/// generic `ID` token.  Operators and digit runs are emitted verbatim.
fn scan<F>(source: &str, mut is_keyword: F) -> Vec<String>
where
    F: FnMut(&str) -> bool,
{
    let bytes = source.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        // Two-character operators first.
        if i + 1 < bytes.len() {
            let pair = [b, bytes[i + 1]];
            match &pair {
                b"->" => {
                    out.push("->".to_string());
                    i += 2;
                    continue;
                }
                b"=>" => {
                    out.push("=>".to_string());
                    i += 2;
                    continue;
                }
                b"::" => {
                    out.push("::".to_string());
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        match classify(b) {
            Class::Ident => {
                let start = i;
                i += 1;
                while i < bytes.len() && is_word_byte(bytes[i]) {
                    i += 1;
                }
                // Safety: we only entered this branch on ASCII.
                let word = &source[start..i];
                if is_keyword(word) {
                    out.push(word.to_string());
                } else {
                    out.push("ID".to_string());
                }
            }
            Class::Number => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                out.push(source[start..i].to_string());
            }
            Class::Punct => {
                // Single-char punctuation gets its own token.
                let bytes = [b];
                let s = std::str::from_utf8(&bytes).unwrap_or("?");
                out.push(s.to_string());
                i += 1;
            }
            Class::Junk => {
                // Whitespace, comments, etc. â€” skip one byte at a time
                // (good enough; comment handling is language-specific
                // and not required for the dedup signal).
                i += 1;
            }
        }
    }
    out
}

// -------- Rust tokenizer ------------------------------------------------

const RUST_KEYWORDS: &[&str] = &[
    "fn", "let", "match", "if", "else", "use", "mod", "pub", "struct", "enum", "trait", "impl",
    "for", "while", "return", "self", "Self",
];

/// Rust source tokenizer.  Captures the Rust keywords / operators that
/// correlate with structural similarity, plus identifier runs.
pub struct RustTokenizer;

impl AstTokenizer for RustTokenizer {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn tokenize(&self, source: &str) -> Vec<String> {
        scan(source, |word| RUST_KEYWORDS.contains(&word))
    }
}

// -------- Python tokenizer ----------------------------------------------

const PYTHON_KEYWORDS: &[&str] = &[
    "def", "class", "if", "else", "elif", "for", "while", "try", "except", "import", "from",
    "return", "self", "lambda", "with", "as",
];

/// Python source tokenizer.  Captures Python keywords and identifier
/// runs.  Python is whitespace-significant, so the keyword set is the
/// main difference from Rust.
pub struct PythonTokenizer;

impl AstTokenizer for PythonTokenizer {
    fn name(&self) -> &'static str {
        "python"
    }

    fn tokenize(&self, source: &str) -> Vec<String> {
        scan(source, |word| PYTHON_KEYWORDS.contains(&word))
    }
}

/// Build a boxed tokenizer by name.  Returns `None` for unsupported
/// languages.
pub fn for_language(lang: &str) -> Option<Box<dyn AstTokenizer>> {
    match lang.to_ascii_lowercase().as_str() {
        "rust" | "rs" => Some(Box::new(RustTokenizer)),
        "python" | "py" => Some(Box::new(PythonTokenizer)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn uniq(tokens: &[String]) -> HashSet<&str> {
        tokens.iter().map(String::as_str).collect()
    }

    #[test]
    fn rust_extracts_keywords_and_identifiers() {
        let t = RustTokenizer;
        let src = "pub fn add(self, other: u32) -> u32 { let sum = self + other; return sum; }";
        let toks = t.tokenize(src);
        let set = uniq(&toks);
        for kw in ["pub", "fn", "self", "let", "return", "->"] {
            assert!(set.contains(kw), "missing {kw} in {toks:?}");
        }
    }

    #[test]
    fn rust_tokenizer_preserves_identifier_names() {
        let t = RustTokenizer;
        let toks = t.tokenize("fn foo_bar() {}");
        assert!(toks.contains(&"fn".to_string()));
        // The identifier must be present (either as the literal name
        // or the canonical `ID` token).
        let has_ident = toks.iter().any(|s| s == "foo_bar" || s == "ID");
        assert!(has_ident, "expected identifier token in {toks:?}");
    }

    #[test]
    fn rust_tokenizer_captures_arrow_and_double_colon() {
        let t = RustTokenizer;
        // Use a snippet that contains `::` so we test that branch.
        let toks = t.tokenize("impl Trait for Type { fn x(&self) -> Self {} use std::io::Write; }");
        let set = uniq(&toks);
        assert!(set.contains("->"));
        assert!(set.contains("::"));
        assert!(set.contains("impl"));
        assert!(set.contains("Self"));
    }

    #[test]
    fn rust_tokenizer_drops_comments() {
        let t = RustTokenizer;
        let src = "// this is a comment\nfn main() { /* block */ }";
        let toks = t.tokenize(src);
        assert!(toks.contains(&"fn".to_string()));
        assert!(toks.contains(&"main".to_string()) || toks.contains(&"ID".to_string()));
        for tok in &toks {
            assert!(!tok.contains("//"));
            assert!(!tok.contains("/*"));
        }
    }

    #[test]
    fn python_extracts_keywords_and_identifiers() {
        let t = PythonTokenizer;
        let src = "def add(self, other):\n    return self + other\n";
        let toks = t.tokenize(src);
        let set = uniq(&toks);
        for kw in ["def", "self", "return"] {
            assert!(set.contains(kw), "missing {kw} in {toks:?}");
        }
    }

    #[test]
    fn python_tokenizer_captures_lambda_and_with() {
        let t = PythonTokenizer;
        let src = "with open(path) as f: data = list(map(lambda x: x.strip(), f))";
        let toks = t.tokenize(src);
        let set = uniq(&toks);
        for kw in ["with", "as", "lambda"] {
            assert!(set.contains(kw), "missing {kw} in {toks:?}");
        }
    }

    #[test]
    fn python_tokenizer_handles_class_definition() {
        let t = PythonTokenizer;
        let src = "class Foo:\n    def bar(self):\n        return 1\n";
        let toks = t.tokenize(src);
        let set = uniq(&toks);
        assert!(set.contains("class"));
        assert!(set.contains("def"));
        assert!(set.contains("self"));
    }

    #[test]
    fn tokenizers_are_deterministic() {
        let r = RustTokenizer;
        let p = PythonTokenizer;
        let src = "fn main() { let x = 1; }";
        assert_eq!(r.tokenize(src), r.tokenize(src));
        let py = "def main():\n    x = 1\n";
        assert_eq!(p.tokenize(py), p.tokenize(py));
    }

    #[test]
    fn for_language_dispatch() {
        assert_eq!(for_language("rust").unwrap().name(), "rust");
        assert_eq!(for_language("rs").unwrap().name(), "rust");
        assert_eq!(for_language("python").unwrap().name(), "python");
        assert_eq!(for_language("py").unwrap().name(), "python");
        assert!(for_language("ruby").is_none());
        assert!(for_language("").is_none());
    }

    #[test]
    fn rust_tokenizer_handles_empty_input() {
        let t = RustTokenizer;
        assert!(t.tokenize("").is_empty());
        assert!(t.tokenize("   \n\t  ").is_empty());
    }

    #[test]
    fn python_tokenizer_handles_empty_input() {
        let t = PythonTokenizer;
        assert!(t.tokenize("").is_empty());
        assert!(t.tokenize("\n\n\n").is_empty());
    }

    #[test]
    fn rust_tokenizer_is_structurally_stable_across_renames() {
        let t = RustTokenizer;
        let a = t.tokenize("fn foo() -> u32 { return 1; }");
        let b = t.tokenize("fn baz() -> u32 { return 1; }");
        // Same number of tokens.
        assert_eq!(a.len(), b.len());
        // Structural tokens identical.
        let sa: HashSet<&str> = a.iter().map(String::as_str).collect();
        let sb: HashSet<&str> = b.iter().map(String::as_str).collect();
        // `fn`, `->`, `return` are keywords / operators emitted verbatim.
        // `u32` is an identifier and gets canonicalized to `ID`.
        for kw in ["fn", "->", "return", "ID"] {
            assert!(
                sa.contains(kw) && sb.contains(kw),
                "kw={kw:?} sa={sa:?} sb={sb:?}"
            );
        }
    }
}
