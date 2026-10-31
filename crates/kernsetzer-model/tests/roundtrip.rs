//! Property test: any `Book` survives `book.json` and the `.kernsetzer`
//! archive unchanged, including unknown fields kept in `extra` maps.

use proptest::prelude::*;
use serde_json::Value;
use setzer_model::presets::TrimSize;
use setzer_model::project::{Project, book_from_json, book_to_json};
use setzer_model::{
    Block, Book, ColumnAlign, Credit, CustomThemeLink, Epigraph, Extra, ImagePlacement, ImageSide,
    Imprint, Inline, Lang, Metadata, Numbering, Section, SectionKind, Series, StyleSettings, Table,
    TableCell, TableColumn,
};
use std::io::Cursor;

fn text() -> impl Strategy<Value = String> {
    // Mostly readable, sometimes anything (control characters, quotes,
    // astral planes): JSON must escape all of it.
    prop_oneof![3 => "[a-zA-ZäöüßÄÖÜ „“‚‘»«–—…’'\"\\\\]{0,12}", 1 => any::<String>()]
}

fn opt_text() -> impl Strategy<Value = Option<String>> {
    proptest::option::of(text())
}

/// Field names of `StyleSettings`, of the `Book` and of any `Block` variant.
/// Unknown fields never use them (the app never writes such an `extra`).
const STYLE_FIELDS: &[&str] = &[
    "theme",
    "trim",
    "params",
    "kinds",
    "params_version",
    "custom_theme",
];
const LINK_FIELDS: &[&str] = &["id", "name", "hash"];
const BOOK_FIELDS: &[&str] = &["format_version", "metadata", "sections", "style"];
const BLOCK_FIELDS: &[&str] = &[
    "type",
    "content",
    "level",
    "blocks",
    "lines",
    "indents",
    "asset",
    "alt",
    "caption",
    "width",
    "placement",
    "side",
    "ordered",
    "items",
    "columns",
    "header_rows",
    "rows",
];
/// The fields of a table's column, and of a table's cell.
const COLUMN_FIELDS: &[&str] = &["align"];
const CELL_FIELDS: &[&str] = &["content", "colspan", "rowspan", "align"];

/// Unknown fields as a newer build might write them: mostly none.
fn extra(known: &'static [&'static str]) -> impl Strategy<Value = Extra> {
    let key = text().prop_filter("an unknown field", move |k| !known.contains(&k.as_str()));
    prop_oneof![
        3 => Just(Extra::new()),
        1 => prop::collection::btree_map(key, json_value(), 1..4)
            .prop_map(|m| m.into_iter().collect()),
    ]
}

fn custom_theme_link() -> impl Strategy<Value = CustomThemeLink> {
    (text(), text(), text(), extra(LINK_FIELDS)).prop_map(|(id, name, hash, extra)| {
        CustomThemeLink {
            id,
            name,
            hash,
            extra,
        }
    })
}

fn with_extra(mut b: Block, e: Extra) -> Block {
    match &mut b {
        Block::Paragraph { extra, .. }
        | Block::Heading { extra, .. }
        | Block::SceneBreak { extra }
        | Block::BlockQuote { extra, .. }
        | Block::Verse { extra, .. }
        | Block::Image { extra, .. }
        | Block::List { extra, .. }
        | Block::PageBreak { extra } => *extra = e,
        Block::Table(table) => table.extra = e,
    }
    b
}

fn inline() -> impl Strategy<Value = Inline> {
    let leaf = prop_oneof![
        4 => text().prop_map(Inline::text),
        1 => Just(Inline::LineBreak),
    ];
    let leaf_inlines = prop::collection::vec(leaf.clone(), 0..4);
    let note_para = (leaf_inlines, extra(BLOCK_FIELDS))
        .prop_map(|(content, e)| with_extra(Block::paragraph(content), e));
    let footnote =
        prop::collection::vec(note_para, 0..3).prop_map(|blocks| Inline::Footnote { blocks });
    prop_oneof![4 => leaf, 1 => footnote].prop_recursive(3, 24, 4, |inner| {
        let content = prop::collection::vec(inner, 0..4);
        prop_oneof![
            content
                .clone()
                .prop_map(|content| Inline::Emphasis { content }),
            content
                .clone()
                .prop_map(|content| Inline::Strong { content }),
            content
                .clone()
                .prop_map(|content| Inline::SmallCaps { content }),
            content
                .clone()
                .prop_map(|content| Inline::Superscript { content }),
            content
                .clone()
                .prop_map(|content| Inline::Subscript { content }),
            (text(), content).prop_map(|(url, content)| Inline::Link { url, content }),
        ]
    })
}

fn inlines() -> impl Strategy<Value = Vec<Inline>> {
    prop::collection::vec(inline(), 0..5)
}

/// Width (out-of-range values included, a hand-edited file), placement and
/// side of a picture.
fn picture() -> impl Strategy<Value = (Option<u8>, ImagePlacement, ImageSide)> {
    (
        proptest::option::of(any::<u8>()),
        prop::sample::select(vec![
            ImagePlacement::Inline,
            ImagePlacement::Beside,
            ImagePlacement::Page,
            ImagePlacement::Bleed,
        ]),
        prop::sample::select(vec![
            ImageSide::Outside,
            ImageSide::Inside,
            ImageSide::Left,
            ImageSide::Right,
        ]),
    )
}

fn column_align() -> impl Strategy<Value = ColumnAlign> {
    prop::sample::select(vec![
        ColumnAlign::Left,
        ColumnAlign::Center,
        ColumnAlign::Right,
    ])
}

/// A span as a hand-edited file may hold it: mostly small, sometimes 0 or
/// huge. The round trip must not normalize any of it.
fn span() -> impl Strategy<Value = u32> {
    prop_oneof![6 => 1u32..4, 1 => Just(0u32), 1 => any::<u32>()]
}

/// A table with nothing assumed: columns that do not match the rows, rows of
/// any length, spans of any size, a header count of any size, unknown fields
/// at the column and the cell (the table's own are added by `with_extra`).
fn table() -> impl Strategy<Value = Block> {
    let column = (column_align(), extra(COLUMN_FIELDS))
        .prop_map(|(align, extra)| TableColumn { align, extra });
    let cell = (
        inlines(),
        span(),
        span(),
        proptest::option::of(column_align()),
        extra(CELL_FIELDS),
    )
        .prop_map(|(content, colspan, rowspan, align, extra)| TableCell {
            content,
            colspan,
            rowspan,
            align,
            extra,
        });
    (
        prop::collection::vec(column, 0..4),
        prop_oneof![3 => 0u32..4, 1 => any::<u32>()],
        prop::collection::vec(prop::collection::vec(cell, 0..4), 0..4),
        proptest::option::of(inlines()),
    )
        .prop_map(|(columns, header_rows, rows, caption)| {
            Block::Table(Table {
                columns,
                header_rows,
                rows,
                caption,
                extra: Extra::new(),
            })
        })
}

fn block() -> impl Strategy<Value = Block> {
    let leaf = prop_oneof![
        6 => inlines().prop_map(Block::paragraph),
        1 => (any::<u8>(), inlines()).prop_map(|(level, content)| Block::heading(level, content)),
        1 => Just(Block::scene_break()),
        1 => Just(Block::page_break()),
        1 => (
            prop::collection::vec(inlines(), 0..4),
            // Any levels, any count: a hand-edited file may not match the
            // lines, and an empty list is the usual case.
            prop::collection::vec(any::<u8>(), 0..6),
        )
            .prop_map(|(lines, indents)| Block::Verse {
                lines,
                indents,
                extra: Extra::new(),
            }),
        2 => table(),
        1 => (text(), text(), proptest::option::of(inlines()), picture())
            .prop_map(|(asset, alt, caption, (width, placement, side))| {
                let mut b = Block::image(asset, alt, caption)
                    .with_placement(placement)
                    .with_side(side);
                if let Some(w) = width {
                    b = b.with_width(w);
                }
                b
            }),
    ];
    let leaf = (leaf, extra(BLOCK_FIELDS)).prop_map(|(b, e)| with_extra(b, e));
    leaf.prop_recursive(2, 16, 3, |inner| {
        let container = prop_oneof![
            prop::collection::vec(inner.clone(), 0..3).prop_map(Block::block_quote),
            (
                any::<bool>(),
                prop::collection::vec(prop::collection::vec(inner, 0..3), 0..3)
            )
                .prop_map(|(ordered, items)| Block::list(ordered, items)),
        ];
        (container, extra(BLOCK_FIELDS)).prop_map(|(b, e)| with_extra(b, e))
    })
}

fn section_kind() -> impl Strategy<Value = SectionKind> {
    use SectionKind::*;
    prop::sample::select(vec![
        HalfTitle,
        TitlePage,
        Copyright,
        Dedication,
        Epigraph,
        TableOfContents,
        Foreword,
        Preface,
        Prologue,
        Part,
        Chapter,
        Epilogue,
        Afterword,
        Acknowledgments,
        AboutTheAuthor,
        AlsoBy,
        Other,
    ])
}

fn numbering() -> impl Strategy<Value = Numbering> {
    prop_oneof![
        Just(Numbering::Auto),
        Just(Numbering::None),
        any::<u32>().prop_map(Numbering::Explicit),
    ]
}

fn section() -> impl Strategy<Value = Section> {
    let epigraph =
        (inlines(), opt_text()).prop_map(|(text, attribution)| Epigraph { text, attribution });
    (
        section_kind(),
        opt_text(),
        numbering(),
        proptest::option::of(epigraph),
        param_map(),
        prop::collection::vec(block(), 0..6),
    )
        .prop_map(
            |(kind, title, numbering, epigraph, style, blocks)| Section {
                style,
                kind,
                title,
                numbering,
                epigraph,
                blocks,
            },
        )
}

fn imprint() -> impl Strategy<Value = Imprint> {
    let credit = (text(), text()).prop_map(|(role, name)| Credit { role, name });
    (
        opt_text(),
        opt_text(),
        opt_text(),
        prop::collection::vec(credit, 0..3),
        opt_text(),
        opt_text(),
    )
        .prop_map(
            |(address, contact, printer, credits, rights, notice)| Imprint {
                address,
                contact,
                printer,
                credits,
                rights,
                notice,
            },
        )
}

fn metadata() -> impl Strategy<Value = Metadata> {
    let series = (text(), proptest::option::of(any::<u32>()))
        .prop_map(|(name, number)| Series { name, number });
    (
        (
            text(),
            opt_text(),
            prop::collection::vec(text(), 0..3),
            text(),
        ),
        (
            proptest::option::of(series),
            opt_text(),
            opt_text(),
            opt_text(),
            opt_text(),
            opt_text(),
        ),
        (opt_text(), imprint()),
    )
        .prop_map(
            |(
                (title, subtitle, authors, lang),
                (series, publisher, isbn_print, isbn_ebook, copyright, edition),
                (cover, imprint),
            )| Metadata {
                title,
                subtitle,
                authors,
                lang: Lang(lang),
                series,
                publisher,
                isbn_print,
                isbn_ebook,
                copyright,
                edition,
                cover,
                imprint,
            },
        )
}

/// Theme parameters and unknown fields: any JSON, including floats and
/// integers beyond `i64`.
fn json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        any::<f64>()
            .prop_filter("JSON has no NaN/inf", |f| f.is_finite())
            .prop_map(Value::from),
        text().prop_map(Value::from),
    ];
    leaf.prop_recursive(2, 12, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::from),
            prop::collection::btree_map(text(), inner, 0..4)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

/// A parameter map (`params`, `kinds[..]`, `Section::style`): any JSON values.
fn param_map() -> impl Strategy<Value = serde_json::Map<String, Value>> {
    prop::collection::btree_map(text(), json_value(), 0..4).prop_map(|m| m.into_iter().collect())
}

fn style() -> impl Strategy<Value = StyleSettings> {
    let dim = any::<f32>().prop_filter("JSON has no NaN/inf", |f| f.is_finite());
    (
        text(),
        dim.clone(),
        dim,
        param_map(),
        prop::collection::btree_map(text(), param_map(), 0..4),
        proptest::option::of(any::<u32>()),
        proptest::option::of(custom_theme_link()),
        extra(STYLE_FIELDS),
    )
        .prop_map(
            |(theme, w, h, params, kinds, params_version, custom_theme, extra)| StyleSettings {
                kinds,
                params_version,
                custom_theme,
                extra,
                theme,
                trim: TrimSize::mm(w, h),
                params,
            },
        )
}

fn book() -> impl Strategy<Value = Book> {
    (
        metadata(),
        prop::collection::vec(section(), 0..5),
        style(),
        extra(BOOK_FIELDS),
    )
        .prop_map(|(metadata, sections, style, extra)| Book {
            sections,
            style,
            extra,
            ..Book::new(metadata)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn book_json_round_trip(b in book()) {
        let json = book_to_json(&b).unwrap();
        prop_assert_eq!(&book_from_json(&json).unwrap(), &b);
        // Writing again is stable (no drift across saves).
        prop_assert_eq!(book_to_json(&book_from_json(&json).unwrap()).unwrap(), json);
    }

    #[test]
    fn setzer_archive_round_trip(b in book()) {
        let p = Project::new(b, Vec::new());
        let bytes = p.write(Cursor::new(Vec::new())).unwrap().into_inner();
        prop_assert_eq!(Project::read(Cursor::new(bytes)).unwrap(), p);
    }
}

/// A `book.json` from a newer build, with unknown fields on the book and
/// on blocks at every depth (top level, quote, list item, footnote), comes
/// back unchanged. `#[serde(flatten)]` inside an internally tagged enum
/// goes through serde's buffered `Content`, so this covers unit variants,
/// and values that are easy to get wrong there: floats, integers beyond
/// `i64`, null, and objects with a `"type"` of their own.
#[test]
fn unknown_fields_round_trip() {
    let json = r#"{
      "format_version": 2,
      "metadata": { "title": "T", "authors": [], "lang": "de" },
      "sections": [{
        "kind": "chapter",
        "numbering": { "type": "auto" },
        "blocks": [
          { "type": "paragraph",
            "content": [
              { "type": "text", "text": "a" },
              { "type": "footnote", "blocks": [
                { "type": "paragraph", "content": [], "x-note": "in a footnote" }
              ] }
            ],
            "x-id": "p1", "x-revision": 3 },
          { "type": "scene-break", "x-ornament": { "type": "glyph", "char": "❦" } },
          { "type": "block-quote",
            "blocks": [ { "type": "paragraph", "content": [], "x-float": 0.1, "x-null": null } ],
            "x-source": [1, "two", false] },
          { "type": "list", "ordered": false,
            "items": [[ { "type": "page-break", "x-big": 18446744073709551615 } ]] },
          { "type": "image", "asset": "a.png", "alt": "", "x-width": -12.5e-3 },
          { "type": "table",
            "columns": [ { "align": "left", "x-col": { "type": "decimal" } } ],
            "rows": [[ { "content": [
                { "type": "text", "text": "a" },
                { "type": "footnote", "blocks": [
                  { "type": "paragraph", "content": [], "x-note": "in a cell" } ] } ],
              "x-cell": 1e3 } ]],
            "x-table": { "type": "paragraph", "n": 18446744073709551615 } }
        ]
      }],
      "style": {
        "theme": "classic",
        "trim": { "width_mm": 120.0, "height_mm": 190.0 },
        "params": {}
      },
      "x-app-state": { "zoom": 1.5, "type": "paragraph" },
      "z": []
    }"#;

    let book = book_from_json(json).unwrap();
    assert_eq!(book.extra["x-app-state"]["zoom"], 1.5);
    assert_eq!(book.extra["z"], Value::Array(vec![]));
    let blocks = &book.sections[0].blocks;
    assert_eq!(blocks[0].extra()["x-id"], "p1");
    assert_eq!(blocks[1].extra()["x-ornament"]["type"], "glyph");
    let Block::Paragraph { content, .. } = &blocks[0] else {
        panic!("{:?}", blocks[0])
    };
    let Inline::Footnote { blocks: note } = &content[1] else {
        panic!("{content:?}")
    };
    assert_eq!(note[0].extra()["x-note"], "in a footnote");
    let Block::BlockQuote { blocks: quoted, .. } = &blocks[2] else {
        panic!("{:?}", blocks[2])
    };
    assert_eq!(quoted[0].extra()["x-float"], 0.1);
    assert_eq!(quoted[0].extra()["x-null"], Value::Null);
    let Block::List { items, .. } = &blocks[3] else {
        panic!("{:?}", blocks[3])
    };
    assert_eq!(items[0][0].extra()["x-big"], u64::MAX);
    // A table keeps unknown fields at the block, a column, a cell and a
    // footnote in a cell.
    let Block::Table(table) = &blocks[5] else {
        panic!("{:?}", blocks[5])
    };
    assert_eq!(blocks[5].extra()["x-table"]["n"], u64::MAX);
    assert_eq!(table.columns[0].extra["x-col"]["type"], "decimal");
    assert_eq!(table.rows[0][0].extra["x-cell"], 1e3);
    let Inline::Footnote { blocks: note } = &table.rows[0][0].content[1] else {
        panic!("{:?}", table.rows[0][0])
    };
    assert_eq!(note[0].extra()["x-note"], "in a cell");
    // Known fields never leak into `extra`.
    assert!(blocks.iter().all(|b| !b.extra().contains_key("type")));

    let out = book_to_json(&book).unwrap();
    let parse = |s: &str| serde_json::from_str::<Value>(s).unwrap();
    assert_eq!(parse(&out), parse(json));
    assert_eq!(book_to_json(&book_from_json(&out).unwrap()).unwrap(), out);
}
