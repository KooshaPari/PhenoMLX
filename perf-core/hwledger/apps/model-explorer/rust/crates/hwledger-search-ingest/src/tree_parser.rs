//! HuggingFace `/api/models/{id}/tree/main` parser.
//!
//! The tree endpoint returns a JSON array of objects that drives tag
//! inference (e.g. presence of `*.gguf` ↔ `quant = GGUF`,
//! `tokenizer.model` ↔ `tokenizer = spm`, …). We keep the parser
//! permissive: any non-array input degrades to `Vec::new()` instead of
//! erroring, because the caller has plenty of other signals to fall back
//! on.

use serde::{Deserialize, Serialize};

/// One entry in a HF model tree.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TreeEntry {
    /// Repo-relative path (e.g. `"qwen2.5-7b-instruct-q4_k_m.gguf"`).
    pub path: String,
    /// File size in bytes, if the upstream payload carries it.
    pub size_bytes: Option<u64>,
    /// HF entry type — typically `"file"` or `"directory"`. We preserve
    /// an `Option` because the field is not always present in practice.
    #[serde(rename = "type")]
    pub r#type: Option<String>,
}

/// Parse a HF tree payload into a normalized [`Vec<TreeEntry>`].
///
/// Falls back to `Vec::new()` for any non-array input so callers can use
/// a single expression regardless of whether the upstream succeeded.
pub fn parse_tree_value(v: &serde_json::Value) -> Vec<TreeEntry> {
    let Some(arr) = v.as_array() else {
        return Vec::new();
    };
    arr.iter().map(tree_entry_from_value).collect()
}

/// Decode a single tree entry. Missing fields stay as `None` / empty —
/// we never reject, because the rest of the pipeline is signal-rich
/// enough to absorb a partial tree.
fn tree_entry_from_value(v: &serde_json::Value) -> TreeEntry {
    let path = v
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_string();
    let size_bytes = v.get("size").and_then(|x| match x {
        serde_json::Value::Number(n) => n.as_u64(),
        serde_json::Value::String(s) => s.parse::<u64>().ok(),
        _ => None,
    });
    let r#type = v
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    TreeEntry {
        path,
        size_bytes,
        r#type,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_tree_value_handles_non_array() {
        assert!(parse_tree_value(&json!({})).is_empty());
        assert!(parse_tree_value(&json!("oops")).is_empty());
        assert!(parse_tree_value(&json!(null)).is_empty());
    }

    #[test]
    fn parse_tree_value_extracts_size_and_type() {
        let v = json!([
            { "path": "a.safetensors", "size": 1024u64, "type": "file" },
            { "path": "b.txt" }
        ]);
        let entries = parse_tree_value(&v);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "a.safetensors");
        assert_eq!(entries[0].size_bytes, Some(1024));
        assert_eq!(entries[0].r#type.as_deref(), Some("file"));
        assert_eq!(entries[1].path, "b.txt");
        assert!(entries[1].size_bytes.is_none());
        assert!(entries[1].r#type.is_none());
    }
}
