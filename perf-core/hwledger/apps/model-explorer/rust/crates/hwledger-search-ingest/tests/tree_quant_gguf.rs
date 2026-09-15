//! `parse_tree_value` end-to-end coverage for a GGUF-heavy tree and
//! the empty-array degenerate case.

use hwledger_search_ingest::parse_tree_value;
use serde_json::json;

#[test]
fn gguf_heavy_tree_contains_gguf_path() {
    let v = json!([
        { "path": ".gitattributes", "size": 1562, "type": "file" },
        { "path": "README.md", "size": 4096, "type": "file" },
        { "path": "config.json", "size": 1024, "type": "file" },
        { "path": "qwen2.5-7b-instruct-q4_k_m.gguf", "size": 4_382_341_120u64, "type": "file" },
        { "path": "qwen2.5-7b-instruct-q8_0.gguf", "size": 7_812_345_678u64, "type": "file" },
        { "path": "tokenizer.model", "size": 2_345_678, "type": "file" }
    ]);
    let entries = parse_tree_value(&v);
    assert_eq!(entries.len(), 6);
    let gguf: Vec<_> = entries.iter().filter(|e| e.path.ends_with(".gguf")).collect();
    assert_eq!(gguf.len(), 2, "expected two .gguf entries");
    assert_eq!(gguf[0].path, "qwen2.5-7b-instruct-q4_k_m.gguf");
    assert_eq!(gguf[0].size_bytes, Some(4_382_341_120));
    assert_eq!(gguf[0].r#type.as_deref(), Some("file"));
    assert!(gguf.iter().any(|e| e.path.contains("q8_0")));
}

#[test]
fn empty_array_returns_empty_vec() {
    let v = json!([]);
    let entries = parse_tree_value(&v);
    assert!(entries.is_empty());
}
