//! Integration test for the ingest convenience wrapper.
//!
//! `IndexedModel` + `upsert_model` is the typed shape that upstream
//! ingesters (the `hwledger-search-ingest` crate, the CLI ingest command)
//! use. We verify that an `IndexedModel` pushed through `upsert_model`
//! is retrievable by a follow-up Tantivy search.

use hwledger_search_index::{upsert_model, IndexedModel, TantivyStore};

#[test]
fn upsert_model_adds_doc_findable_by_subsequent_search() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = TantivyStore::open(dir.path()).expect("open");

    let m = IndexedModel {
        id: "org/Model-X".into(),
        name: "Model X".into(),
        org: "org".into(),
        kind: "instruct".into(),
        family: "fam".into(),
        arch: "gqa".into(),
        quants: vec!["gguf".into(), "gptq".into()],
        card_snippet: "Model X is a small language model for unit tests.".into(),
    };

    upsert_model(&store, &m).expect("upsert_model");
    store.commit().expect("commit");

    // The free-text body should be retrievable.
    let hits = store.search("language", 5).expect("search language");
    assert_eq!(
        hits.len(),
        1,
        "expected exactly one hit for `language`, got {:?}",
        hits
    );
    assert_eq!(hits[0].id, "org/Model-X");

    // The name token should also be retrievable, and rank alongside it.
    let name_hits = store.search("Model", 5).expect("search model");
    assert!(
        name_hits.iter().any(|h| h.id == "org/Model-X"),
        "expected `org/Model-X` in hits for `Model`, got {:?}",
        name_hits
    );

    // And the quants sidecar should reflect the joined list.
    let quants = store.quants_for_id("org/Model-X").expect("quants sidecar");
    assert!(
        quants.contains(&"gguf".to_string()) && quants.contains(&"gptq".to_string()),
        "sidecar must contain both gguf and gptq, got {:?}",
        quants
    );

    // And the kind sidecar must reflect what we sent.
    assert_eq!(
        store.kind_for_id("org/Model-X").as_deref(),
        Some("instruct")
    );
}