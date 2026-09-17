//! Integration test: parse the sample GSD fixture and assert shape.

use agileplus_spec_harmonizer::{parsers::Parser, parsers::gsd::GsdParser};
use std::fs;

#[test]
fn parses_fixture_gsd() {
    let text = fs::read_to_string("fixtures/gsd_sample.md").expect("fixture");
    let out = GsdParser.parse(&text).expect("parse");
    assert_eq!(out.len(), 3, "expected 3 GSD tasks");
    assert_eq!(out[0].title, "Bootstrap repo");
    assert_eq!(out[0].acceptance.len(), 3);
    assert_eq!(out[0].acceptance.iter().filter(|a| a.done).count(), 1);
    assert_eq!(out[1].title, "Add CLI entrypoint");
    assert_eq!(out[1].acceptance.iter().filter(|a| a.done).count(), 2);
    assert_eq!(out[2].title, "Persist state");
    assert_eq!(out[2].acceptance.iter().filter(|a| a.done).count(), 2);
}
