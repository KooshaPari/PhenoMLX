// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Fuzz target for mock SQL fragment sanitization / tokenization.
//
// Exercises a self-contained SQL tokenizer that handles common SQL dialect
// features. Catches panics in:
//   - UTF-8 validation on arbitrary byte input
//   - Unclosed single-quoted string literals (quote tracking)
//   - Nested block comment boundaries (/* /* inner */ */)
//   - Line comment spanning (-- through EOL)
//   - Keyword detection for SELECT / INSERT / UPDATE / DELETE
//   - Escaped quote sequences inside string literals ('')
//   - Mixed comment/string interleaving at token boundaries
//
// The tokenizer does NOT connect to a database; it operates purely on
// in-memory byte slices and is safe to fuzz.

#![no_main]

use libfuzzer_sys::fuzz_target;

/// Minimal SQL fragment tokenizer that classifies input into tokens.
///
/// Returns a list of token kinds found in `input`. Handles:
/// - String literals delimited by single quotes with '' escaping
/// - Line comments (`--` to end of line)
/// - Block comments (`/* ... */`, including nesting)
/// - Case-insensitive keyword matching for DML statements
fn tokenize_sql(input: &str) -> Vec<&'static str> {
    let mut tokens: Vec<&'static str> = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // ---- Block comment: /* ... */ (supports nesting) ----
        if bytes[i..].starts_with(b"/*") {
            let mut depth: u32 = 1;
            let mut j = i + 2;
            while j < bytes.len() && depth > 0 {
                if bytes[j..].starts_with(b"/*") {
                    depth += 1;
                    j += 2;
                } else if bytes[j..].starts_with(b"*/") {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            tokens.push("block_comment");
            i = j;
            continue;
        }

        // ---- Line comment: -- to end of line ----
        if bytes[i..].starts_with(b"--") {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j] != b'\n' {
                j += 1;
            }
            tokens.push("line_comment");
            i = j;
            continue;
        }

        // ---- String literal: '...' with '' escaping ----
        if bytes[i] == b'\'' {
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b'\'' {
                    // Check for escaped quote ''
                    if j + 1 < bytes.len() && bytes[j + 1] == b'\'' {
                        j += 2;
                        continue;
                    }
                    j += 1;
                    break;
                }
                j += 1;
            }
            tokens.push("string_literal");
            i = j;
            continue;
        }

        // ---- Whitespace ----
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // ---- Word boundary: check keywords / identifiers ----
        if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
            {
                i += 1;
            }
            let word = &input[start..i];
            let upper = word.to_ascii_uppercase();
            match upper.as_str() {
                "SELECT" => tokens.push("SELECT"),
                "INSERT" => tokens.push("INSERT"),
                "UPDATE" => tokens.push("UPDATE"),
                "DELETE" => tokens.push("DELETE"),
                "FROM" => tokens.push("FROM"),
                "WHERE" => tokens.push("WHERE"),
                "SET" => tokens.push("SET"),
                "INTO" => tokens.push("INTO"),
                "VALUES" => tokens.push("VALUES"),
                _ => tokens.push("identifier"),
            }
            continue;
        }

        // ---- Single-character token (operator / punctuation) ----
        i += 1;
    }

    tokens
}

fuzz_target!(|data: &[u8]| {
    // ---- UTF-8 gate: reject non-UTF-8 before string operations ----
    let Ok(s) = std::str::from_utf8(data) else { return };

    #[allow(unused_variables)]
    let tokens = tokenize_sql(s);

    // ---- Edge-case: empty / whitespace-only / very long input ----
    // tokenize_sql handles these without panicking.
});
