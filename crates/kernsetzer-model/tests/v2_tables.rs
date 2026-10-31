//! The table block in format version 2. `tests/fixtures/v2/tables.json` (2026-10-28) is frozen like the
//! other version 2 files; never edit or re-generate it. Its text is Austen's
//! (*Pride and Prejudice*, public domain). It holds
//! three tables: one with a two-row header and a group heading (`colspan`), a
//! body cell with `rowspan`, a cell `align` override, three column
//! alignments, an empty cell, a caption, a footnote in a cell and unknown
//! fields at the table, a column, a cell and the footnote's block; a plain
//! one with nothing optional; and a damaged one (no columns, a span of 0, a
//! rowspan past the last row, a colspan of 40, a row with no cell, a header
//! count beyond the rows) that must read and write unchanged.

use serde_json::{Value, json};
use setzer_model::presets::TrimSize;
use setzer_model::project::{book_from_json, book_to_json};
use setzer_model::{
    Block, Book, ColumnAlign as A, Extra, Inline, Lang, Metadata, Numbering, Section, SectionKind,
    StyleSettings, Table, TableCell, TableColumn,
};
use std::path::Path;

fn fixture() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v2/tables.json");
    std::fs::read_to_string(path).unwrap()
}

fn t(s: &str) -> Inline {
    Inline::text(s)
}

fn c(s: &str) -> TableCell {
    if s.is_empty() {
        TableCell::new(vec![])
    } else {
        TableCell::text(s)
    }
}

fn extra(key: &str, value: Value) -> Extra {
    let mut e = Extra::new();
    e.insert(key.into(), value);
    e
}

/// The first table, built the way the fixture says it.
fn assembly() -> Table {
    let note = Inline::Footnote {
        blocks: vec![Block::Paragraph {
            content: vec![t("R. W. Chapman’s edition of 1923")],
            extra: extra("x-note", json!("in a cell")),
        }],
    };
    let mut banner = c("the assembly").with_span(2, 1).with_align(A::Center);
    banner.extra = extra("x-cell", json!({"type": "banner", "n": 1}));
    let mut first = TableColumn::new(A::Left);
    first.extra = extra("x-width", json!("4em"));
    Table {
        columns: vec![
            first,
            TableColumn::new(A::Right),
            TableColumn::new(A::Right),
        ],
        header_rows: 2,
        rows: vec![
            vec![c("Mr. Bingley").with_span(1, 2), banner],
            vec![c("report"), c("party")],
            vec![c("ladies").with_span(1, 2), c("twelve"), c("six")],
            vec![c("seven"), c("")],
            vec![
                c("altogether"),
                c("five").with_align(A::Left),
                TableCell::new(vec![t("six"), note]),
            ],
        ],
        caption: Some(vec![t("Netherfield")]),
        extra: extra(
            "x-table",
            json!({"style": "booktabs", "type": "paragraph", "big": u64::MAX}),
        ),
    }
}

fn names() -> Table {
    Table {
        columns: vec![TableColumn::new(A::Left), TableColumn::new(A::Left)],
        header_rows: 0,
        rows: vec![
            vec![c("Jane"), c("the eldest Miss Bennet")],
            vec![c("Lydia"), c("the youngest")],
        ],
        caption: None,
        extra: Extra::new(),
    }
}

fn damaged() -> Table {
    Table {
        columns: vec![],
        header_rows: 9,
        rows: vec![
            vec![c("Elizabeth").with_span(0, 5), c("Mary").with_span(40, 1)],
            vec![],
            vec![c("Catherine"), c("Lydia"), c("Jane")],
        ],
        caption: None,
        extra: Extra::new(),
    }
}

/// The content of the fixture in today's model.
fn expected() -> Book {
    Book {
        format_version: setzer_model::FORMAT_VERSION,
        metadata: Metadata {
            title: "Pride and Prejudice".into(),
            authors: vec!["Jane Austen".into()],
            lang: Lang("en".into()),
            ..Metadata::default()
        },
        sections: vec![Section {
            style: Default::default(),
            kind: SectionKind::Chapter,
            title: Some("Chapter I".into()),
            numbering: Numbering::Auto,
            epigraph: None,
            blocks: vec![
                Block::paragraph(vec![t(
                    "It is a truth universally acknowledged, that a single man in \
                     possession of a good fortune, must be in want of a wife.",
                )]),
                Block::Table(assembly()),
                Block::paragraph(vec![t("Mr. Bennet made no answer.")]),
                Block::Table(names()),
                Block::Table(damaged()),
            ],
        }],
        style: StyleSettings {
            trim: TrimSize::mm(152.4, 228.6),
            ..StyleSettings::default()
        },
        extra: Extra::new(),
    }
}

#[test]
fn v2_tables_load() {
    let book = book_from_json(&fixture()).unwrap();
    assert_eq!(book.format_version, 2);
    assert_eq!(book, expected());
    // Unknown fields at the four levels are there to be kept.
    let blocks = &book.sections[0].blocks;
    assert_eq!(blocks[1].extra()["x-table"]["big"], u64::MAX);
    let Block::Table(t1) = &blocks[1] else {
        panic!("{:?}", blocks[1])
    };
    assert_eq!(t1.columns[0].extra["x-width"], "4em");
    assert_eq!(t1.rows[0][1].extra["x-cell"]["type"], "banner");
    let Inline::Footnote { blocks: note } = &t1.rows[4][2].content[1] else {
        panic!("{:?}", t1.rows[4][2])
    };
    assert_eq!(note[0].extra()["x-note"], "in a cell");
}

#[test]
fn v2_tables_round_trip_unchanged() {
    let book = book_from_json(&fixture()).unwrap();
    let back = book_from_json(&book_to_json(&book).unwrap()).unwrap();
    assert_eq!(back, book);
    // Written as read: the fixture is what the writer makes (it was made
    // with it), damaged table included. Reading never normalizes.
    let parse = |s: &str| serde_json::from_str::<Value>(s).unwrap();
    assert_eq!(parse(&book_to_json(&book).unwrap()), parse(&fixture()));
    let written = parse(&book_to_json(&book).unwrap());
    let blocks = written["sections"][0]["blocks"].as_array().unwrap();
    // Defaults are not written, spans and the header count are as stored.
    assert!(blocks[3].get("header_rows").is_none());
    assert!(blocks[3].get("caption").is_none());
    assert!(blocks[3]["rows"][0][0].get("colspan").is_none());
    assert_eq!(blocks[4]["header_rows"], 9);
    assert_eq!(blocks[4]["rows"][0][0]["colspan"], 0);
    assert_eq!(blocks[4]["rows"][0][1]["colspan"], 40);
}

#[test]
fn v2_tables_lay_out() {
    let book = book_from_json(&fixture()).unwrap();
    let blocks = &book.sections[0].blocks;
    let Block::Table(first) = &blocks[1] else {
        panic!("{:?}", blocks[1])
    };
    let l = first.layout();
    assert_eq!((l.columns, l.rows, l.header_rows), (3, 5, 2));
    let at = |row: usize, column: usize| {
        l.cells
            .iter()
            .find(|c| c.row == row && c.column == column)
            .unwrap_or_else(|| panic!("no cell at {row},{column}"))
    };
    // The group heading covers two columns, centred; the label two rows.
    assert_eq!((at(0, 1).colspan, at(0, 1).align), (2, A::Center));
    assert_eq!((at(0, 0).rowspan, at(2, 0).rowspan), (2, 2));
    // `seven` stands in column 1 because `ladies` covers column 0; the empty
    // cell is stored, the override is kept.
    assert_eq!(at(3, 1).source, Some((3, 0)));
    assert_eq!(at(3, 2).source, Some((3, 1)));
    assert_eq!(at(4, 1).align, A::Left);
    assert_eq!(at(4, 2).align, A::Right);
    // The damaged table lays out as a grid and loses nothing.
    let Block::Table(bad) = &blocks[4] else {
        panic!("{:?}", blocks[4])
    };
    let l = bad.layout();
    // No columns: the width is the widest row's, 1 + 40.
    assert_eq!((l.columns, l.rows, l.header_rows), (41, 3, 3));
    let text: Vec<String> = l
        .cells
        .iter()
        .map(|c| Inline::plain_text(&c.content()))
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(text, ["Elizabeth", "Mary", "Catherine", "Lydia", "Jane"]);
}
