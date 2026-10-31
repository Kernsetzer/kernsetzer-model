//! The custom-theme link in format version 2: `StyleSettings.custom_theme`
//! is reserved and unused.
//! `tests/fixtures/v2/custom-theme.json` (2026-10-28) is frozen like the
//! other version 2 files; never edit or re-generate it.

use serde_json::json;
use setzer_model::project::{book_from_json, book_to_json};
use setzer_model::{Block, Book, Inline, Metadata, Section};
use std::path::Path;

fn fixture_named(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/v2")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn custom_theme_link_loads() {
    let book = book_from_json(&fixture_named("custom-theme.json")).unwrap();
    assert_eq!(book.format_version, 2);
    let link = book.style.custom_theme.as_ref().unwrap();
    assert_eq!(link.id, "harbour-series");
    assert_eq!(link.name, "Pride and Prejudice");
    assert!(link.hash.starts_with("sha256:"));
    // The unknown link field is kept, and the book's own design is separate.
    assert_eq!(link.extra["future-link-field"], json!({"depth": [1, 2]}));
    assert_eq!(link.extra.len(), 1);
    assert_eq!(book.style.params["scene-break"], "dinkus");
    assert!(book.style.extra.is_empty());
}

#[test]
fn custom_theme_link_round_trips() {
    let book = book_from_json(&fixture_named("custom-theme.json")).unwrap();
    let written = book_to_json(&book).unwrap();
    let back = book_from_json(&written).unwrap();
    assert_eq!(back, book);
    let value: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(value["style"]["custom_theme"]["id"], "harbour-series");
    assert_eq!(
        value["style"]["custom_theme"]["future-link-field"],
        json!({"depth": [1, 2]})
    );
}

#[test]
fn other_v2_fixtures_have_no_link() {
    for name in ["pictures.json", "style-scopes.json"] {
        let book = book_from_json(&fixture_named(name)).unwrap();
        assert_eq!(book.style.custom_theme, None, "{name}");
    }
}

#[test]
fn an_absent_link_is_not_written() {
    let mut book = Book::new(Metadata::default());
    book.sections.push(Section::chapter(
        "Eins",
        vec![Block::paragraph(vec![Inline::text("Text.")])],
    ));
    let written: serde_json::Value = serde_json::from_str(&book_to_json(&book).unwrap()).unwrap();
    assert!(written["style"].get("custom_theme").is_none());
}
