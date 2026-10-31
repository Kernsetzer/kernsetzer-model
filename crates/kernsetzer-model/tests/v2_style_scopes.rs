//! Design scopes in format version 2: the fields
//! `StyleSettings.kinds` and `Section.style` are reserved and unused, and so
//! are `StyleSettings.params_version` and `extra`.
//! `tests/fixtures/v2/style-scopes.json` (2026-10-26, extended
//! 2026-10-27 with the marker and an unknown style field before it reached
//! main) is frozen like the other version 2 files; never edit or re-generate
//! it.

use serde_json::json;
use setzer_model::project::{book_from_json, book_to_json};
use setzer_model::{
    Block, Book, KIND_BACK_MATTER, KIND_CHAPTERS, KIND_FRONT_MATTER, KIND_PARTS, Metadata, Section,
};
use std::path::Path;

fn fixture() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v2/style-scopes.json");
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn style_scopes_load() {
    let book = book_from_json(&fixture()).unwrap();
    assert_eq!(book.format_version, 2);
    let kinds = &book.style.kinds;
    assert_eq!(kinds.len(), 5);
    assert_eq!(kinds[KIND_CHAPTERS]["initial"], "raised");
    assert_eq!(kinds[KIND_CHAPTERS]["chapter-start"], "recto");
    assert_eq!(kinds[KIND_PARTS]["chapter-ornament"], "fleuron");
    assert_eq!(kinds[KIND_FRONT_MATTER]["folio"], "none");
    assert_eq!(kinds[KIND_BACK_MATTER]["head-verso"], "none");
    // The book's own parameters are separate from the kinds.
    assert_eq!(book.style.params["scene-break"], "dinkus");
    assert_eq!(book.style.params_version, Some(1));
    assert_eq!(
        book.style.extra["future-style-field"],
        json!({"depth": [1, 2]})
    );
    assert_eq!(book.style.extra.len(), 1);

    assert_eq!(book.sections[0].style["initial"], "none");
    assert_eq!(book.sections[0].style["scene-break"], "rule");
    assert_eq!(book.sections[1].style["head-recto"], "none");
    assert_eq!(
        book.sections[1].style["future-knob"],
        json!({"depth": [1, 2]})
    );
}

#[test]
fn style_scopes_round_trip_keeps_unknown_kind() {
    let book = book_from_json(&fixture()).unwrap();
    let back = book_from_json(&book_to_json(&book).unwrap()).unwrap();
    assert_eq!(back, book);
    assert_eq!(
        back.style.kinds["poetry"]["note"],
        "von einer neueren Version"
    );
    // The marker is a named field and the unknown style field stays in `extra`.
    let written: serde_json::Value = serde_json::from_str(&book_to_json(&book).unwrap()).unwrap();
    assert_eq!(written["style"]["params_version"], 1);
    assert_eq!(
        written["style"]["future-style-field"],
        json!({"depth": [1, 2]})
    );
    assert_eq!(back.style.params_version, Some(1));
    assert_eq!(
        back.style.extra["future-style-field"],
        json!({"depth": [1, 2]})
    );
}

#[test]
fn pictures_fixture_has_no_marker() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v2/pictures.json");
    let book = book_from_json(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(book.style.params_version, None);
    assert!(book.style.extra.is_empty());
}

#[test]
fn empty_scopes_are_not_written() {
    let mut book = Book::new(Metadata::default());
    book.sections.push(Section::chapter(
        "Eins",
        vec![Block::paragraph(vec![setzer_model::Inline::text("Text.")])],
    ));
    let written: serde_json::Value = serde_json::from_str(&book_to_json(&book).unwrap()).unwrap();
    assert!(written["style"].get("kinds").is_none());
    assert!(written["style"].get("params_version").is_none());
    assert_eq!(
        written["style"].as_object().unwrap().keys().count(),
        3,
        "only theme, trim and params"
    );
    assert!(written["sections"][0].get("style").is_none());

    // A book read without the fields has none, and writes none back.
    let plain: Book = book_from_json(&book_to_json(&book).unwrap()).unwrap();
    assert!(plain.style.kinds.is_empty());
    assert_eq!(plain.style.params_version, None);
    assert!(plain.style.extra.is_empty());
    assert!(plain.sections[0].style.is_empty());
}
