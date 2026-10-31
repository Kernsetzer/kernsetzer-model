//! Format version 2 files must open forever. `tests/fixtures/v2/` is frozen
//! from the day each file was written; never edit or re-generate one. A
//! later change to the version 2 shape is a bump to 3.
//!
//! `pictures.json` (2026-10-26): every placement, every side,
//! widths 10, 55 and 100, a missing width, and unknown fields in `extra`.

use serde_json::json;
use setzer_model::project::{book_from_json, book_to_json};
use setzer_model::{Block, ImagePlacement as P, ImageSide as S, Section};
use std::path::Path;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/v2")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn v2_pictures_load() {
    let book = book_from_json(&fixture("pictures.json")).unwrap();
    assert_eq!(book.format_version, 2);
    let blocks = &book.sections[0].blocks;
    let pic = |i: usize| match &blocks[i] {
        Block::Image {
            width,
            placement,
            side,
            ..
        } => (*width, *placement, *side),
        other => panic!("not a picture: {other:?}"),
    };
    assert_eq!(pic(0), (None, P::Inline, S::Outside));
    assert_eq!(pic(1), (Some(10), P::Inline, S::Outside));
    assert_eq!(pic(2), (Some(55), P::Beside, S::Outside));
    assert_eq!(pic(3), (Some(40), P::Beside, S::Inside));
    assert_eq!(pic(4), (Some(35), P::Beside, S::Left));
    assert_eq!(pic(5), (Some(30), P::Beside, S::Right));
    assert_eq!(pic(6), (Some(100), P::Page, S::Outside));
    assert_eq!(pic(7), (None, P::Bleed, S::Outside));
    // The side survives a placement that does not read it.
    assert_eq!(pic(8), (Some(55), P::Inline, S::Left));
    assert_eq!(pic(9), (Some(100), P::Beside, S::Outside));

    // Read through the clamp: 55 beside the text is 50, 100 beside is 50.
    assert_eq!(blocks[1].image_width(), Some(10));
    assert_eq!(blocks[2].image_width(), Some(50));
    assert_eq!(blocks[8].image_width(), Some(55));
    assert_eq!(blocks[9].image_width(), Some(50));
    assert_eq!(blocks[0].image_width(), None);

    // Unknown fields are kept.
    assert_eq!(
        blocks[9].extra()["future_crop"],
        json!({"x": 1, "y": [2, 3]})
    );
    assert_eq!(blocks[9].extra()["note"], "später");
    let _: &Section = &book.sections[0];
}

#[test]
fn v2_pictures_round_trip() {
    let book = book_from_json(&fixture("pictures.json")).unwrap();
    let back = book_from_json(&book_to_json(&book).unwrap()).unwrap();
    assert_eq!(back, book);
    // Defaults are not written: the explicit `"placement": "inline"` of the
    // fixture's second picture reads fine but is not written back.
    let written: serde_json::Value = serde_json::from_str(&book_to_json(&book).unwrap()).unwrap();
    let blocks = written["sections"][0]["blocks"].as_array().unwrap();
    assert!(blocks[0].get("width").is_none());
    assert!(blocks[1].get("placement").is_none());
    assert!(blocks[2].get("side").is_none());
}
