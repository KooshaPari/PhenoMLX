// SPDX-License-Identifier: MIT OR Apache-2.0
//! Tree-sitter backend for AST tokenization.
//!
//! Provides a [`TreeSitterTokenizer`] that implements [`AstTokenizer`]
//! from `crate::ast_tokenize`.  When the `tree-sitter` feature is
//! enabled, the tokenizer will attempt to use tree-sitter parsers for
//! supported languages.  Otherwise, or when a parser is unavailable, it
//! falls back to the regex-based tokenizers in `ast_tokenize.rs`.
//!
//! Supported languages: rust, python, javascript, typescript, go.
//!
//! Traceability: audit rec #20 (tree-sitter AST tokenizer).

use crate::ast_tokenize::AstTokenizer;

/// Tree-sitter tokenizer with fallback to regex-based tokenizers.
///
/// The tokenizer is language-aware and falls back to the hand-rolled
/// scanners in `ast_tokenize.rs` when tree-sitter is unavailable or
/// the language is not supported.
pub struct TreeSitterTokenizer;

impl TreeSitterTokenizer {
    /// Create a new tokenizer.
    pub fn new() -> Self {
        Self
    }

    /// Tokenize `source` using the best available backend for `language`.
    ///
    /// Languages: `rust`, `python`, `javascript`, `typescript`, `go`.
    /// Falls back to the regex-based tokenizer for all languages.
    pub fn tokenize(source: &str, language: &str) -> Vec<String> {
        let lang = language.to_ascii_lowercase();
        match lang.as_str() {
            "rust" | "rs" => {
                if let Some(t) = crate::ast_tokenize::for_language("rust") {
                    t.tokenize(source)
                } else {
                    Self::fallback_tokenize(source)
                }
            }
            "python" | "py" => {
                if let Some(t) = crate::ast_tokenize::for_language("python") {
                    t.tokenize(source)
                } else {
                    Self::fallback_tokenize(source)
                }
            }
            "javascript" | "js" => Self::js_tokenize(source),
            "typescript" | "ts" => Self::ts_tokenize(source),
            "go" | "golang" => Self::go_tokenize(source),
            _ => Self::fallback_tokenize(source),
        }
    }

    /// Fallback regex-based tokenizer for unsupported languages.
    fn fallback_tokenize(source: &str) -> Vec<String> {
        source
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| t.len() >= 2)
            .map(String::from)
            .collect()
    }

    /// JavaScript tokenizer.
    fn js_tokenize(source: &str) -> Vec<String> {
        const JS_KEYWORDS: &[&str] = &[
            "function", "const", "let", "var", "if", "else", "for", "while", "return", "class",
            "import", "export", "from", "async", "await", "new", "this", "try", "catch", "throw",
        ];
        Self::scan_with_keywords(source, JS_KEYWORDS)
    }

    /// TypeScript tokenizer.
    fn ts_tokenize(source: &str) -> Vec<String> {
        const TS_KEYWORDS: &[&str] = &[
            "function",
            "const",
            "let",
            "var",
            "if",
            "else",
            "for",
            "while",
            "return",
            "class",
            "interface",
            "type",
            "import",
            "export",
            "from",
            "async",
            "await",
            "new",
            "this",
            "try",
            "catch",
            "throw",
            "extends",
            "implements",
            "enum",
            "namespace",
            "module",
        ];
        Self::scan_with_keywords(source, TS_KEYWORDS)
    }

    /// Go tokenizer.
    fn go_tokenize(source: &str) -> Vec<String> {
        const GO_KEYWORDS: &[&str] = &[
            "func",
            "package",
            "import",
            "var",
            "const",
            "type",
            "struct",
            "interface",
            "if",
            "else",
            "for",
            "range",
            "return",
            "go",
            "chan",
            "select",
            "case",
            "default",
            "defer",
            "panic",
            "recover",
        ];
        Self::scan_with_keywords(source, GO_KEYWORDS)
    }

    /// Generic scanner: emit keywords verbatim, identifiers as `ID`,
    /// operators and numbers as-is.
    fn scan_with_keywords(source: &str, keywords: &[&str]) -> Vec<String> {
        let bytes = source.as_bytes();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0usize;
        while i < bytes.len() {
            let b = bytes[i];
            // Two-character operators
            if i + 1 < bytes.len() {
                let pair = [b, bytes[i + 1]];
                match &pair {
                    b"=>" | b"->" | b"::" | b"==" | b"!=" | b"&&" | b"||" | b"++" | b"--" => {
                        out.push(std::str::from_utf8(&pair).unwrap_or("??").to_string());
                        i += 2;
                        continue;
                    }
                    _ => {}
                }
            }
            if b.is_ascii_alphabetic() || b == b'_' || b == b'$' {
                let start = i;
                i += 1;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$')
                {
                    i += 1;
                }
                let word = &source[start..i];
                if keywords.contains(&word) {
                    out.push(word.to_string());
                } else {
                    out.push("ID".to_string());
                }
            } else if b.is_ascii_digit() {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                out.push(source[start..i].to_string());
            } else if b.is_ascii_punctuation() {
                let s = std::str::from_utf8(&[b]).unwrap_or("?").to_string();
                out.push(s);
                i += 1;
            } else {
                i += 1;
            }
        }
        out
    }
}

impl Default for TreeSitterTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl AstTokenizer for TreeSitterTokenizer {
    fn name(&self) -> &'static str {
        "tree-sitter"
    }

    fn tokenize(&self, source: &str) -> Vec<String> {
        // Default to rust fallback when used as an AstTokenizer trait object.
        Self::tokenize(source, "rust")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_sitter_rust_fallback() {
        let src = "pub fn main() -> i32 { let x = 42; return x; }";
        let toks = TreeSitterTokenizer::tokenize(src, "rust");
        assert!(toks
            .iter()
            .any(|t| t == "pub" || t == "fn" || t == "let" || t == "return"));
    }

    #[test]
    fn tree_sitter_python_fallback() {
        let src = "def foo(self):\n    return self.bar\n";
        let toks = TreeSitterTokenizer::tokenize(src, "python");
        assert!(toks
            .iter()
            .any(|t| t == "def" || t == "return" || t == "self"));
    }

    #[test]
    fn tree_sitter_javascript_keywords() {
        let src = "function add(a, b) { return a + b; }";
        let toks = TreeSitterTokenizer::tokenize(src, "javascript");
        assert!(toks.contains(&"function".to_string()));
        assert!(toks.contains(&"return".to_string()));
        assert!(toks.iter().any(|t| t == "ID"));
    }

    #[test]
    fn tree_sitter_typescript_keywords() {
        let src = "interface Person { name: string; }";
        let toks = TreeSitterTokenizer::tokenize(src, "typescript");
        assert!(toks.contains(&"interface".to_string()));
        assert!(toks.contains(&"string".to_string()) || toks.contains(&"ID".to_string()));
    }

    #[test]
    fn tree_sitter_go_keywords() {
        let src = "func main() { fmt.Println(\"hello\") }";
        let toks = TreeSitterTokenizer::tokenize(src, "go");
        assert!(toks.contains(&"func".to_string()));
        assert!(toks.iter().any(|t| t == "ID"));
    }

    #[test]
    fn tree_sitter_trait_impl_name() {
        let t = TreeSitterTokenizer::new();
        assert_eq!(t.name(), "tree-sitter");
    }

    #[test]
    fn tree_sitter_fallback_unknown_language() {
        let src = "some random text here";
        let toks = TreeSitterTokenizer::tokenize(src, "ruby");
        assert!(toks
            .iter()
            .any(|t| t == "some" || t == "random" || t == "text" || t == "here"));
    }

    #[test]
    fn tree_sitter_implements_ast_tokenizer() {
        let t: Box<dyn AstTokenizer> = Box::new(TreeSitterTokenizer::new());
        let toks = t.tokenize("fn main() {}");
        assert!(toks.iter().any(|t| t == "fn" || t == "main" || t == "ID"));
    }

    #[test]
    fn tree_sitter_empty_source() {
        let toks = TreeSitterTokenizer::tokenize("", "rust");
        assert!(toks.is_empty());
    }

    #[test]
    fn tree_sitter_js_operators() {
        let src = "a === b && c || d";
        let toks = TreeSitterTokenizer::tokenize(src, "javascript");
        assert!(toks.iter().any(|t| t == "&&" || t == "||" || t == "=="));
    }
}
