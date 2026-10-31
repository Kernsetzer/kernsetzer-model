//! `Table::layout()` on damaged tables: the hand-damaged
//! values in `tests/fixtures/tables/*.json` with the layout each must give
//! (free to grow: add a file), and properties on random tables, damaged ones
//! included.

use proptest::prelude::*;
use serde_json::Value;
use setzer_model::{Block, ColumnAlign, Inline, Table, TableCell, TableColumn, TableLayout};
use std::path::Path;

fn text_of(inlines: &[Inline]) -> String {
    Inline::plain_text(inlines)
}

/// Every position is covered exactly once, every row has a cell of its own,
/// no span crosses the edge or the header boundary.
fn assert_complete(l: &TableLayout) {
    let mut seen = vec![vec![0u32; l.columns]; l.rows];
    for cell in &l.cells {
        assert!(cell.rowspan >= 1 && cell.colspan >= 1, "{cell:?}");
        assert!(cell.row + cell.rowspan <= l.rows, "{cell:?}");
        assert!(cell.column + cell.colspan <= l.columns, "{cell:?}");
        if cell.row < l.header_rows {
            assert!(cell.row + cell.rowspan <= l.header_rows, "{cell:?}");
        }
        for row in &mut seen[cell.row..cell.row + cell.rowspan] {
            for n in &mut row[cell.column..cell.column + cell.colspan] {
                *n += 1;
            }
        }
    }
    assert!(seen.iter().flatten().all(|&n| n == 1), "{seen:?}");
    for r in 0..l.rows {
        assert!(l.cells.iter().any(|c| c.row == r), "row {r} has no cell");
    }
    assert!(l.header_rows <= l.rows);
    assert_eq!(l.aligns.len(), l.columns);
}

#[test]
fn damaged_tables_lay_out_as_their_fixtures_say() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tables");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    names.sort();
    assert!(names.len() >= 7, "the damaged-table fixtures are missing");
    for path in names {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let json: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let Block::Table(table) = serde_json::from_value(json["table"].clone()).unwrap() else {
            panic!("{name}: not a table");
        };
        let expect = &json["expect"];
        let l = table.layout();
        assert_complete(&l);
        assert_eq!(
            l.columns as u64,
            expect["columns"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(l.rows as u64, expect["rows"].as_u64().unwrap(), "{name}");
        assert_eq!(
            l.header_rows as u64,
            expect["header_rows"].as_u64().unwrap(),
            "{name}"
        );
        // [row, column, rowspan, colspan, text, source]
        let got: Vec<Value> = l
            .cells
            .iter()
            .map(|c| {
                serde_json::json!([
                    c.row,
                    c.column,
                    c.rowspan,
                    c.colspan,
                    text_of(&c.content()),
                    c.source.map(|(r, i)| [r, i])
                ])
            })
            .collect();
        assert_eq!(
            Value::Array(got),
            expect["cells"],
            "{name}: {}",
            json["note"]
        );
        if let Some(aligns) = expect.get("aligns") {
            let got: Vec<String> = l
                .cells
                .iter()
                .map(|c| {
                    serde_json::to_value(c.align)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .into()
                })
                .collect();
            assert_eq!(serde_json::to_value(got).unwrap(), *aligns, "{name}");
        }
        // Reading never normalizes: the file's table is as stored, and the
        // normal form lays out as itself.
        let n = table.normalized();
        assert_eq!(n.normalized(), n, "{name}");
        assert_complete(&n.layout());
    }
}

// ----- properties -----------------------------------------------------------

fn word() -> impl Strategy<Value = String> {
    "[a-z]{1,3}"
}

fn content() -> impl Strategy<Value = Vec<Inline>> {
    let note = word().prop_map(|w| Inline::Footnote {
        blocks: vec![Block::paragraph(vec![Inline::text(w)])],
    });
    let leaf = prop_oneof![
        5 => word().prop_map(Inline::text),
        1 => Just(Inline::LineBreak),
        1 => note,
        1 => word().prop_map(|w| Inline::Emphasis { content: vec![Inline::text(w)] }),
    ];
    prop::collection::vec(leaf, 0..3)
}

fn span() -> impl Strategy<Value = u32> {
    prop_oneof![8 => 0u32..5, 1 => Just(u32::MAX), 1 => 5u32..100]
}

fn align() -> impl Strategy<Value = ColumnAlign> {
    prop::sample::select(vec![
        ColumnAlign::Left,
        ColumnAlign::Center,
        ColumnAlign::Right,
    ])
}

fn cell() -> impl Strategy<Value = TableCell> {
    (content(), span(), span(), proptest::option::of(align())).prop_map(
        |(content, colspan, rowspan, align)| {
            let mut c = TableCell::new(content).with_span(colspan, rowspan);
            c.align = align;
            c
        },
    )
}

/// A stored table with nothing assumed: columns that may be missing or
/// short, rows of any length, spans of any size, a header count of any size.
fn table() -> impl Strategy<Value = Table> {
    (
        prop::collection::vec(align().prop_map(TableColumn::new), 0..6),
        prop_oneof![8 => 0u32..6, 1 => Just(u32::MAX)],
        prop::collection::vec(prop::collection::vec(cell(), 0..6), 0..7),
    )
        .prop_map(|(columns, header_rows, rows)| Table {
            columns,
            header_rows,
            rows,
            caption: None,
            extra: Default::default(),
        })
}

/// The text leaves of `inlines` in reading order, footnotes included.
fn leaves(inlines: &[Inline], out: &mut Vec<String>) {
    for i in inlines {
        match i {
            Inline::Text { text } => out.push(text.clone()),
            Inline::Emphasis { content }
            | Inline::Strong { content }
            | Inline::SmallCaps { content }
            | Inline::Superscript { content }
            | Inline::Subscript { content }
            | Inline::Link { content, .. } => leaves(content, out),
            Inline::Footnote { blocks } => {
                for b in blocks {
                    if let Block::Paragraph { content, .. } = b {
                        leaves(content, out);
                    }
                }
            }
            Inline::LineBreak => {}
        }
    }
}

/// The comparable part of a layout (its cells without where they came from).
fn shape(l: &TableLayout) -> Vec<(usize, usize, usize, usize, ColumnAlign, Vec<Inline>)> {
    l.cells
        .iter()
        .map(|c| (c.row, c.column, c.rowspan, c.colspan, c.align, c.content()))
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Every position covered once, every row has a cell, no span crosses
    /// the edge or the header boundary, and the width is the columns'.
    #[test]
    fn layout_is_a_complete_grid(t in table()) {
        let l = t.layout();
        assert_complete(&l);
        if !t.columns.is_empty() {
            prop_assert_eq!(l.columns, t.columns.len());
        }
        prop_assert_eq!(l.rows, t.rows.len());
        prop_assert!(l.header_rows <= t.header_rows as usize);
    }

    /// No cell text is lost, and the text keeps its order.
    #[test]
    fn layout_keeps_all_text(t in table()) {
        let (mut stored, mut laid) = (Vec::new(), Vec::new());
        for cell in t.rows.iter().flatten() {
            leaves(&cell.content, &mut stored);
        }
        for cell in &t.layout().cells {
            leaves(&cell.content(), &mut laid);
        }
        prop_assert_eq!(stored, laid);
    }

    /// `normalized` is a fixed point and lays out as the table it came from.
    #[test]
    fn normalized_lays_out_as_itself(t in table()) {
        let n = t.normalized();
        let (a, b) = (t.layout(), n.layout());
        prop_assert_eq!(a.columns, b.columns);
        prop_assert_eq!(a.rows, b.rows);
        prop_assert_eq!(a.header_rows, b.header_rows);
        prop_assert_eq!(&a.aligns, &b.aligns);
        prop_assert_eq!(shape(&a), shape(&b));
        prop_assert!(b.cells.iter().all(|c| c.source.is_some()));
        prop_assert_eq!(n.normalized(), n);
    }
}
