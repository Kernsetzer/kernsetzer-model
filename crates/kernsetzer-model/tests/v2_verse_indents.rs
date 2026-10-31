//! Indents per verse line in format version 2:
//! `Block::Verse.indents`, one level per line, skipped when empty.
//! `tests/fixtures/v2/verse-indents.json` (2026-10-28) is frozen like the
//! other version 2 files; never edit or re-generate it. Its text is
//! Carroll's "How doth the little crocodile" (*Alice's Adventures in
//! Wonderland*, Project Gutenberg #11, public domain): a stanza pair with
//! every second line in; the second stanza alone with no levels at all (the
//! shape of every file written before indents); a list of levels shorter than
//! its lines; one longer, with an unknown field in `extra`; and a level far
//! beyond the deepest.

use serde_json::json;
use setzer_model::project::{book_from_json, book_to_json};
use setzer_model::{Block, MAX_VERSE_INDENT};
use std::path::Path;

fn fixture() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v2/verse-indents.json");
    std::fs::read_to_string(path).unwrap()
}

/// The level of every line of a verse block, read through the accessor.
fn levels(b: &Block) -> Vec<u8> {
    let Block::Verse { lines, .. } = b else {
        panic!("not verse: {b:?}");
    };
    (0..lines.len()).map(|i| b.verse_indent(i)).collect()
}

#[test]
fn v2_verse_indents_load() {
    let book = book_from_json(&fixture()).unwrap();
    assert_eq!(book.format_version, 2);
    let blocks = &book.sections[0].blocks;
    // Two stanzas and the gap between them; the gap is flush.
    assert_eq!(levels(&blocks[0]), [0, 1, 0, 1, 0, 0, 1, 0, 1]);
    // No levels stored: all flush.
    assert!(matches!(&blocks[1], Block::Verse { indents, .. } if indents.is_empty()));
    assert_eq!(levels(&blocks[1]), [0, 0, 0, 0]);
    // A damaged file never panics: a missing level is 0 ...
    assert_eq!(levels(&blocks[2]), [0, 1, 0, 0]);
    // ... and levels beyond the lines are ignored.
    assert_eq!(levels(&blocks[3]), [0, 1, 0, 2]);
    // A level beyond the deepest is read as the deepest, and kept as stored.
    assert_eq!(levels(&blocks[4]), [0, MAX_VERSE_INDENT, 0, 1]);
    assert!(matches!(&blocks[4], Block::Verse { indents, .. } if indents[1] == 200));
    // A line that does not exist, and a block that is not verse.
    assert_eq!(blocks[0].verse_indent(9), 0);
    assert_eq!(blocks[0].verse_indent(usize::MAX), 0);
    // Unknown fields are kept.
    assert_eq!(blocks[3].extra()["future_metre"], json!({"feet": [4, 3]}));
}

#[test]
fn v2_verse_indents_round_trip() {
    let book = book_from_json(&fixture()).unwrap();
    let back = book_from_json(&book_to_json(&book).unwrap()).unwrap();
    assert_eq!(back, book);
    // Stored as given, damaged or not; a verse without levels writes none.
    let written: serde_json::Value = serde_json::from_str(&book_to_json(&book).unwrap()).unwrap();
    let blocks = written["sections"][0]["blocks"].as_array().unwrap();
    assert_eq!(blocks[0]["indents"], json!([0, 1, 0, 1, 0, 0, 1, 0, 1]));
    assert!(blocks[1].get("indents").is_none());
    assert_eq!(blocks[2]["indents"], json!([0, 1]));
    assert_eq!(blocks[3]["indents"], json!([0, 1, 0, 2, 3, 4]));
    assert_eq!(blocks[4]["indents"], json!([0, 200, 0, 1]));
}
