//! Format version 1 files must open forever. `tests/fixtures/v1/` holds a
//! `book.json` and a `.setzer` written by the first build with the zip
//! container (2026-10-19); they are frozen. Never re-generate them: when the
//! schema changes, add a migration and a `v2/` fixture next to these, and
//! update `expected()` only to describe the same content in the new model.

use setzer_model::presets::TrimSize;
use setzer_model::project::{Project, book_from_json};
use setzer_model::{
    Block, Book, Epigraph, Imprint, Inline, Lang, Metadata, Numbering, Section, SectionKind,
    Series, StyleSettings,
};
use std::path::Path;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/v1")
        .join(name)
}

fn t(s: &str) -> Inline {
    Inline::text(s)
}

fn para(content: Vec<Inline>) -> Block {
    Block::paragraph(content)
}

/// The content of the v1 fixtures, in today's model: every block and
/// inline kind, every numbering, all metadata fields.
fn expected() -> Book {
    let mut params = serde_json::Map::new();
    params.insert("body-size".into(), 10.5.into());
    params.insert("drop-cap".into(), true.into());
    params.insert("ornament".into(), "❦".into());
    Book {
        // A version 1 file migrates to the current version when read.
        format_version: setzer_model::FORMAT_VERSION,
        extra: Default::default(),
        metadata: Metadata {
            title: "Ein Landarzt".into(),
            subtitle: Some("Kleine Erzählungen".into()),
            authors: vec!["A. Autorin".into(), "B. Autor".into()],
            lang: Lang("de-AT".into()),
            series: Some(Series {
                name: "Proben".into(),
                number: Some(2),
            }),
            publisher: Some("Eigenverlag".into()),
            isbn_print: Some("978-3-00-000000-0".into()),
            isbn_ebook: Some("978-3-00-000001-7".into()),
            copyright: Some("© 2026 A. Autorin".into()),
            edition: Some("1. Auflage".into()),
            cover: None,
            imprint: Imprint::default(),
        },
        sections: vec![
            Section {
                style: Default::default(),
                kind: SectionKind::Dedication,
                title: None,
                numbering: Numbering::None,
                epigraph: None,
                blocks: vec![para(vec![t("Für M.")])],
            },
            Section {
                style: Default::default(),
                kind: SectionKind::Part,
                title: Some("Teil 1: Das nächste Dorf".into()),
                numbering: Numbering::Auto,
                epigraph: None,
                blocks: vec![],
            },
            Section {
                style: Default::default(),
                kind: SectionKind::Chapter,
                title: Some("Vor dem Gesetz".into()),
                numbering: Numbering::Explicit(1),
                epigraph: Some(Epigraph {
                    text: vec![t("So ist es.")],
                    attribution: Some("Franz Kafka".into()),
                }),
                blocks: vec![
                    para(vec![
                        t("Noch standen treu die "),
                        Inline::Emphasis {
                            content: vec![t("Pferde")],
                        },
                        t(" an ihren "),
                        Inline::Strong {
                            content: vec![Inline::SmallCaps {
                                content: vec![t("Plätzen")],
                            }],
                        },
                        Inline::Footnote {
                            blocks: vec![para(vec![t("Dort sitzt er Tage und Jahre.")])],
                        },
                        t(", H"),
                        Inline::Subscript {
                            content: vec![t("2")],
                        },
                        t("O und 10"),
                        Inline::Superscript {
                            content: vec![t("3")],
                        },
                        t(" "),
                        Inline::Link {
                            url: "https://example.org".into(),
                            content: vec![t("Link")],
                        },
                        Inline::LineBreak,
                        t("„Gut genug“ – so ist es…"),
                    ]),
                    Block::scene_break(),
                    Block::heading(2, vec![t("Auf der Galerie")]),
                    Block::block_quote(vec![para(vec![t("Ich schwang mich aufs Pferd.")])]),
                    Block::verse(vec![
                        vec![t("Wärme und Geruch wie von Pferden kam hervor")],
                        vec![t("Eine trübe Stallaterne schwankte drin an einem Seil")],
                        vec![],
                        vec![t("Das Dienstmädchen stand neben mir")],
                    ]),
                    Block::image("map.png", "Karte", Some(vec![t("Das nächste Dorf")])),
                    Block::list(
                        true,
                        vec![vec![para(vec![t("Eins")])], vec![para(vec![t("Zwei")])]],
                    ),
                    Block::page_break(),
                ],
            },
            Section {
                style: Default::default(),
                kind: SectionKind::Other,
                title: Some("Zwischenspiel".into()),
                numbering: Numbering::None,
                epigraph: None,
                blocks: vec![Block::list(false, vec![vec![para(vec![t("Punkt")])]])],
            },
            Section {
                style: Default::default(),
                kind: SectionKind::AboutTheAuthor,
                title: Some("Eine kaiserliche Botschaft".into()),
                numbering: Numbering::Auto,
                epigraph: None,
                blocks: vec![para(vec![t("Nun bin ich aber Arzt.")])],
            },
        ],
        style: StyleSettings {
            kinds: Default::default(),
            params_version: None,
            custom_theme: None,
            extra: Default::default(),
            theme: "classic".into(),
            trim: TrimSize::DE_12X19,
            params,
        },
    }
}

#[test]
fn v1_book_json_loads() {
    let json = std::fs::read_to_string(fixture("book.json")).unwrap();
    assert_eq!(book_from_json(&json).unwrap(), expected());
}

#[test]
fn v1_project_opens() {
    let p = Project::open(&fixture("Probe.setzer")).unwrap();
    assert_eq!(p.book, expected());
    // `meta` is kept as read; the file becomes version 2 when saved.
    assert_eq!(p.meta.format_version, 1);
    assert_eq!(p.book.format_version, 2);
    assert_eq!(p.meta.created_with, "setzer 0.1.0");
    assert_eq!(p.meta.modified.as_deref(), Some("2026-10-19T12:00:00Z"));
    let ids: Vec<_> = p.assets.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, ["map.png"]);
    let map = &p.assets[0];
    assert_eq!(map.media_type, "image/png");
    assert_eq!((map.width_px, map.height_px), (Some(3), Some(2)));
    assert_eq!(map.dpi.map(f32::round), Some(300.0));
}

#[test]
fn v1_project_saves_as_version_2() {
    let p = Project::open(&fixture("Probe.setzer")).unwrap();
    let bytes = p
        .write(std::io::Cursor::new(Vec::new()))
        .unwrap()
        .into_inner();
    let back = Project::read(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(back.meta.format_version, 2);
    assert_eq!(back.book, expected());
    assert_eq!(back.assets, p.assets);
}
