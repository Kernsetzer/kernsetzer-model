//! The semantic document: `Book` → `Section`s → `Block`s → `Inline`s.

use crate::table::Table;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Fields in `book.json` that this build does not know, kept as read and
/// written back on save, so a file saved by a newer build loses
/// nothing when an older one re-saves it. Code never puts known field names
/// here; leave it empty when building a value.
pub type Extra = serde_json::Map<String, serde_json::Value>;

/// A whole book as imported and edited by the author.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Book {
    pub format_version: u32,
    pub metadata: Metadata,
    /// Front matter, body and back matter in reading order.
    pub sections: Vec<Section>,
    pub style: StyleSettings,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Book {
    pub fn new(metadata: Metadata) -> Self {
        Self {
            format_version: crate::FORMAT_VERSION,
            metadata,
            sections: Vec::new(),
            style: StyleSettings::default(),
            extra: Extra::new(),
        }
    }

    /// Body chapters only (what gets numbered).
    pub fn chapters(&self) -> impl Iterator<Item = &Section> {
        self.sections
            .iter()
            .filter(|s| s.kind == SectionKind::Chapter)
    }

    /// The number of each section, by index: chapters and parts are
    /// counted apart, `Auto` counts on from the last one of its kind,
    /// `Explicit(n)` is `n` and the count goes on from it. Other kinds and
    /// `Numbering::None` have none. Print, EPUB, pre-flight and the app all
    /// number from this.
    pub fn section_numbers(&self) -> Vec<Option<SectionNumber>> {
        let (mut chapters, mut parts) = (0u32, 0u32);
        self.sections
            .iter()
            .map(|s| {
                let count = match s.kind {
                    SectionKind::Chapter => &mut chapters,
                    SectionKind::Part => &mut parts,
                    _ => return None,
                };
                let expected = count.saturating_add(1);
                let number = match s.numbering {
                    Numbering::Auto => expected,
                    Numbering::Explicit(n) => n,
                    Numbering::None => return None,
                };
                *count = number;
                Some(SectionNumber { number, expected })
            })
            .collect()
    }

    pub fn block(&self, at: BlockRef) -> Option<&Block> {
        self.sections.get(at.section)?.blocks.get(at.block)
    }

    pub fn block_mut(&mut self, at: BlockRef) -> Option<&mut Block> {
        self.sections.get_mut(at.section)?.blocks.get_mut(at.block)
    }
}

/// Where a top-level block is: `sections[section].blocks[block]`. Edits
/// (M4 `book_apply`) address sections by index and blocks with this; a
/// change inside a quote, list or footnote replaces its top-level block.
/// Positions, not ids stored in the file: ops run one after another on the
/// book held in Rust, so a position is exact when its op runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockRef {
    pub section: usize,
    pub block: usize,
}

/// BCP 47 language tag, e.g. `en`, `en-GB`, `de`, `de-AT`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lang(pub String);

impl Lang {
    /// Primary subtag, lowercased (`de-AT` → `de`).
    pub fn primary(&self) -> String {
        self.0
            .split(['-', '_'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
    }

    /// The tag as BCP 47 writes it, without spaces around it and with its
    /// subtags joined by `-` (`en_GB` → `en-GB`). `None` when it is no
    /// language tag as Kernsetzer reads one: a primary language subtag of two
    /// or three letters, then subtags of one to eight letters and digits
    /// (`de-AT`, `en_GB`, `sr-Latn`). Pre-flight reports any other language
    /// as `INVALID_LANGUAGE`.
    pub fn tag(&self) -> Option<String> {
        let mut parts = self.0.trim().split(['-', '_']);
        let primary = parts.next().unwrap_or_default();
        let mut tag = primary.to_string();
        if !(2..=3).contains(&primary.len()) || !primary.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        for p in parts {
            if !(1..=8).contains(&p.len()) || !p.chars().all(|c| c.is_ascii_alphanumeric()) {
                return None;
            }
            tag.push('-');
            tag.push_str(p);
        }
        Some(tag)
    }

    /// The language the book is set in, print and EPUB alike: this one as
    /// [`Lang::tag`] writes it, or English (the default) where it is no
    /// language tag, so that a language typed wrong never stops a render.
    pub fn effective(&self) -> Lang {
        self.tag().map(Lang).unwrap_or_default()
    }
}

impl Default for Lang {
    fn default() -> Self {
        Lang("en".into())
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub lang: Lang,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series: Option<Series>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn_print: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn_ebook: Option<String>,
    /// Free text for the copyright page, e.g. "© 2026 Jane Doe".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
    /// Asset id of the front cover image, used for the EPUB cover.
    /// Print covers are made outside Kernsetzer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
    #[serde(default, skip_serializing_if = "Imprint::is_empty")]
    pub imprint: Imprint,
}

/// The rest of the copyright page (DE: Impressum); the © line, ISBNs and
/// edition are in [`Metadata`]. German press law asks for the name and
/// address of the publisher (`Metadata::publisher`; a self-publisher names
/// themselves) and of the printer. Values only, printed as written; the
/// theme adds labels ("Druck:", "Printed by") in the book's language.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Imprint {
    /// The publisher's postal address; lines separated by `\n`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Email or website.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    /// The printer's name and address, e.g. "Amazon Distribution GmbH,
    /// Leipzig".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub printer: Option<String>,
    /// Cover design, editing, proofreading, image credits, in print order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub credits: Vec<Credit>,
    /// E.g. "All rights reserved." or "Alle Rechte vorbehalten."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rights: Option<String>,
    /// Anything else: a fiction disclaimer, the Deutsche Nationalbibliothek
    /// notice. Paragraphs separated by a blank line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

impl Imprint {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// One credit line on the copyright page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Credit {
    /// As printed, in the book's language: "Lektorat", "Cover design".
    pub role: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Series {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
}

/// What role a section plays. Drives page layout (display pages, running
/// heads, numbering) and EPUB semantics (`epub:type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SectionKind {
    // Front matter
    HalfTitle,
    TitlePage,
    Copyright,
    Dedication,
    Epigraph,
    TableOfContents,
    Foreword,
    Preface,
    Prologue,
    // Body
    Part,
    Chapter,
    // Back matter
    Epilogue,
    Afterword,
    Acknowledgments,
    AboutTheAuthor,
    AlsoBy,
    /// Anything else, with the heading as given.
    Other,
}

impl SectionKind {
    pub fn is_front_matter(self) -> bool {
        use SectionKind::*;
        matches!(
            self,
            HalfTitle
                | TitlePage
                | Copyright
                | Dedication
                | Epigraph
                | TableOfContents
                | Foreword
                | Preface
        )
    }

    pub fn is_back_matter(self) -> bool {
        use SectionKind::*;
        matches!(self, Afterword | Acknowledgments | AboutTheAuthor | AlsoBy)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub kind: SectionKind,
    /// Heading as displayed, without the "Chapter N" label (that is
    /// generated by the theme from `numbering`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub numbering: Numbering,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epigraph: Option<Epigraph>,
    /// Design choices for this section alone, the narrowest scope. Reserved
    /// in format version 2; values are stored as given and checked by the
    /// code that lays out the book, not here.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub style: serde_json::Map<String, serde_json::Value>,
    pub blocks: Vec<Block>,
}

impl Section {
    pub fn chapter(title: impl Into<String>, blocks: Vec<Block>) -> Self {
        Self {
            kind: SectionKind::Chapter,
            title: Some(title.into()),
            numbering: Numbering::Auto,
            epigraph: None,
            style: serde_json::Map::new(),
            blocks,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "value")]
pub enum Numbering {
    /// Numbered in sequence with the other chapters.
    #[default]
    Auto,
    /// Not numbered (e.g. an interlude).
    None,
    /// Number given explicitly in the manuscript.
    Explicit(u32),
}

/// A chapter's or part's number, from [`Book::section_numbers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionNumber {
    /// The number shown.
    pub number: u32,
    /// The number that follows the last one of its kind (what `Auto`
    /// gives); an `Explicit` number other than this is out of sequence.
    pub expected: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Epigraph {
    pub text: Vec<Inline>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
}

/// Every variant carries `extra` (unknown fields, see [`Extra`]); build
/// blocks with the constructors below and match with `..`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum Block {
    Paragraph {
        content: Vec<Inline>,
        #[serde(flatten)]
        extra: Extra,
    },
    /// A sub-heading inside a section (rare in fiction). `level` counts
    /// from the section title: 1 is the first level below it (Markdown
    /// `##`; in .docx the heading style one below the chapter style).
    Heading {
        level: u8,
        content: Vec<Inline>,
        #[serde(flatten)]
        extra: Extra,
    },
    SceneBreak {
        #[serde(flatten)]
        extra: Extra,
    },
    BlockQuote {
        blocks: Vec<Block>,
        #[serde(flatten)]
        extra: Extra,
    },
    /// Lines kept as written; stanzas separated by empty `Vec`s.
    Verse {
        lines: Vec<Vec<Inline>>,
        /// The indent level of each line (0 is flush), by position in
        /// `lines`; empty when no line is indented, and then not written.
        /// May be shorter or longer than `lines` in a damaged file: read it
        /// through [`Block::verse_indent`], never by index.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        indents: Vec<u8>,
        #[serde(flatten)]
        extra: Extra,
    },
    Image {
        asset: String,
        #[serde(default)]
        alt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caption: Option<Vec<Inline>>,
        /// Width as a percentage of the text block's width, 10–100 (the app
        /// offers steps of 5). `None`: automatic, today's fit, or 40 % beside
        /// the text. Read through [`Block::image_width`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<u8>,
        #[serde(default, skip_serializing_if = "ImagePlacement::is_inline")]
        placement: ImagePlacement,
        /// Which side a picture beside the text stands on; read only for
        /// `Beside`, kept when the placement changes.
        #[serde(default, skip_serializing_if = "ImageSide::is_outside")]
        side: ImageSide,
        #[serde(flatten)]
        extra: Extra,
    },
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
        #[serde(flatten)]
        extra: Extra,
    },
    /// A table. Its JSON is flat: the table's own fields and its
    /// `extra` stand next to `"type": "table"`. Draw it through
    /// [`Table::layout`], never by indexing `rows`.
    Table(Table),
    PageBreak {
        #[serde(flatten)]
        extra: Extra,
    },
}

impl Block {
    pub fn paragraph(content: Vec<Inline>) -> Self {
        Block::Paragraph {
            content,
            extra: Extra::new(),
        }
    }

    pub fn heading(level: u8, content: Vec<Inline>) -> Self {
        Block::Heading {
            level,
            content,
            extra: Extra::new(),
        }
    }

    pub fn scene_break() -> Self {
        Block::SceneBreak {
            extra: Extra::new(),
        }
    }

    pub fn block_quote(blocks: Vec<Block>) -> Self {
        Block::BlockQuote {
            blocks,
            extra: Extra::new(),
        }
    }

    pub fn verse(lines: Vec<Vec<Inline>>) -> Self {
        Block::Verse {
            lines,
            indents: Vec::new(),
            extra: Extra::new(),
        }
    }

    /// Set the indent level of each line of a verse block, by position in
    /// `lines` (other blocks are returned unchanged). All zeros is stored as
    /// none, so a verse without indents writes the same JSON as before.
    /// Levels are stored as given and capped on read.
    pub fn with_indents(mut self, levels: Vec<u8>) -> Self {
        if let Block::Verse { indents, .. } = &mut self {
            *indents = if levels.iter().all(|&l| l == 0) {
                Vec::new()
            } else {
                levels
            };
        }
        self
    }

    /// The indent level of verse line `line`: 0 when the block has no level
    /// for it (none stored, a shorter list in a damaged file, a block that
    /// is not verse), at most [`MAX_VERSE_INDENT`]; levels beyond the lines
    /// are ignored. Always read the levels through this.
    pub fn verse_indent(&self, line: usize) -> u8 {
        match self {
            Block::Verse { lines, indents, .. } if line < lines.len() => indents
                .get(line)
                .map_or(0, |&level| level.min(MAX_VERSE_INDENT)),
            _ => 0,
        }
    }

    pub fn image(
        asset: impl Into<String>,
        alt: impl Into<String>,
        caption: Option<Vec<Inline>>,
    ) -> Self {
        Block::Image {
            asset: asset.into(),
            alt: alt.into(),
            caption,
            width: None,
            placement: ImagePlacement::Inline,
            side: ImageSide::Outside,
            extra: Extra::new(),
        }
    }

    /// Set a picture's width (percent of the text width); other blocks are
    /// returned unchanged. The value is stored as given and clamped on read.
    pub fn with_width(mut self, percent: u8) -> Self {
        if let Block::Image { width, .. } = &mut self {
            *width = Some(percent);
        }
        self
    }

    pub fn with_placement(mut self, to: ImagePlacement) -> Self {
        if let Block::Image { placement, .. } = &mut self {
            *placement = to;
        }
        self
    }

    pub fn with_side(mut self, to: ImageSide) -> Self {
        if let Block::Image { side, .. } = &mut self {
            *side = to;
        }
        self
    }

    /// A picture's width as a percentage of the text block, clamped for a
    /// hand-edited file: 10–100, and 25–50 beside the text. `None` for
    /// automatic width and for other blocks.
    pub fn image_width(&self) -> Option<u8> {
        match self {
            Block::Image {
                width: Some(w),
                placement,
                ..
            } => Some(match placement {
                ImagePlacement::Beside => (*w).clamp(25, 50),
                _ => (*w).clamp(10, 100),
            }),
            _ => None,
        }
    }

    pub fn list(ordered: bool, items: Vec<Vec<Block>>) -> Self {
        Block::List {
            ordered,
            items,
            extra: Extra::new(),
        }
    }

    pub fn table(table: Table) -> Self {
        Block::Table(table)
    }

    pub fn page_break() -> Self {
        Block::PageBreak {
            extra: Extra::new(),
        }
    }

    /// Unknown fields read with this block.
    pub fn extra(&self) -> &Extra {
        match self {
            Block::Paragraph { extra, .. }
            | Block::Heading { extra, .. }
            | Block::SceneBreak { extra }
            | Block::BlockQuote { extra, .. }
            | Block::Verse { extra, .. }
            | Block::Image { extra, .. }
            | Block::List { extra, .. }
            | Block::PageBreak { extra } => extra,
            Block::Table(table) => &table.extra,
        }
    }
}

/// The deepest verse indent level the model reads; the importers
/// stop there too. A level is a step in from the verse's own indent; the
/// themes decide how far.
pub const MAX_VERSE_INDENT: u8 = 6;

/// Where a picture stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImagePlacement {
    /// In the flow of the text, centred, where the author put it.
    #[default]
    Inline,
    /// Beside the text, which wraps round it.
    Beside,
    /// On a page of its own, inside the margins.
    Page,
    /// On a page of its own, to the trimmed edge and into the bleed.
    Bleed,
}

impl ImagePlacement {
    pub fn is_inline(&self) -> bool {
        *self == ImagePlacement::Inline
    }
}

/// The side a `Beside` picture stands on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageSide {
    /// The page's outer edge: right on a recto, left on a verso.
    #[default]
    Outside,
    /// The binding edge.
    Inside,
    Left,
    Right,
}

impl ImageSide {
    pub fn is_outside(&self) -> bool {
        *self == ImageSide::Outside
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum Inline {
    Text { text: String },
    Emphasis { content: Vec<Inline> },
    Strong { content: Vec<Inline> },
    SmallCaps { content: Vec<Inline> },
    Superscript { content: Vec<Inline> },
    Subscript { content: Vec<Inline> },
    Link { url: String, content: Vec<Inline> },
    Footnote { blocks: Vec<Block> },
    LineBreak,
}

impl Inline {
    pub fn text(s: impl Into<String>) -> Self {
        Inline::Text { text: s.into() }
    }

    /// Plain text content, ignoring formatting and footnotes.
    pub fn plain_text(inlines: &[Inline]) -> String {
        let mut out = String::new();
        for i in inlines {
            match i {
                Inline::Text { text } => out.push_str(text),
                Inline::Emphasis { content }
                | Inline::Strong { content }
                | Inline::SmallCaps { content }
                | Inline::Superscript { content }
                | Inline::Subscript { content }
                | Inline::Link { content, .. } => out.push_str(&Self::plain_text(content)),
                Inline::LineBreak => out.push(' '),
                Inline::Footnote { .. } => {}
            }
        }
        out
    }
}

/// Everything about appearance the author chooses. Themes interpret the
/// parameters; unknown parameters are kept for forward compatibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleSettings {
    pub theme: String,
    pub trim: crate::presets::TrimSize,
    #[serde(default)]
    pub params: serde_json::Map<String, serde_json::Value>,
    /// Design choices for a kind of section (between the book's `params` and
    /// one section's `style`), keyed by [`KIND_CHAPTERS`], [`KIND_PARTS`],
    /// [`KIND_FRONT_MATTER`] and [`KIND_BACK_MATTER`]. Reserved in format
    /// version 2. Unknown keys are kept, since a newer build may add kinds;
    /// values are stored as given.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub kinds: BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
    /// Which rules `params`, `kinds` and the sections' `style` were set under.
    /// Absent means the older rules, under which a theme ignores a value it
    /// does not declare, so leftovers from an earlier theme may sit in the
    /// maps. Newer builds write 1 on new books and when they normalize an old
    /// one. Reserved in format version 2; nothing reads or writes it yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params_version: Option<u32>,
    /// The custom theme this book's design was taken from: a link only,
    /// since the book keeps its own copy of the values and art. Reserved in
    /// format version 2; nothing reads or writes it yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_theme: Option<CustomThemeLink>,
    /// Fields written by newer builds, kept on round-trip.
    #[serde(flatten)]
    pub extra: Extra,
}

/// The link from a book to the custom theme it was designed from
/// (`StyleSettings::custom_theme`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomThemeLink {
    /// The theme's id in the author's library.
    pub id: String,
    /// The name the author gave it, shown when the library lacks the theme.
    pub name: String,
    /// Hash of the theme file's content when the book last took the design
    /// from it; a different hash in the library means the theme has changed.
    pub hash: String,
    /// Fields written by newer builds, kept on round-trip.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Key in [`StyleSettings::kinds`] for the numbered and unnumbered chapters.
/// The mapping from `SectionKind` to these keys is left to the caller.
pub const KIND_CHAPTERS: &str = "chapters";
/// Key for part pages.
pub const KIND_PARTS: &str = "parts";
/// Key for the sections before the first chapter: dedication, epigraph,
/// foreword and the like.
pub const KIND_FRONT_MATTER: &str = "front-matter";
/// Key for the sections after the last chapter: afterword, acknowledgments,
/// about the author, also-by.
pub const KIND_BACK_MATTER: &str = "back-matter";

impl Default for StyleSettings {
    fn default() -> Self {
        Self {
            theme: "classic".into(),
            trim: crate::presets::TrimSize::US_6X9,
            params: serde_json::Map::new(),
            kinds: BTreeMap::new(),
            params_version: None,
            custom_theme: None,
            extra: Extra::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picture_defaults_are_not_written() {
        let json = serde_json::to_value(Block::image("a.png", "x", None)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type": "image", "asset": "a.png", "alt": "x"})
        );
        let b = Block::image("a.png", "x", None)
            .with_width(55)
            .with_placement(ImagePlacement::Beside)
            .with_side(ImageSide::Inside);
        let json = serde_json::to_value(&b).unwrap();
        assert_eq!(json["width"], 55);
        assert_eq!(json["placement"], "beside");
        assert_eq!(json["side"], "inside");
        assert_eq!(serde_json::from_value::<Block>(json).unwrap(), b);
    }

    #[test]
    fn picture_width_is_clamped_on_read() {
        let w = |width: u8, placement| {
            Block::image("a", "", None)
                .with_width(width)
                .with_placement(placement)
                .image_width()
        };
        assert_eq!(w(0, ImagePlacement::Inline), Some(10));
        assert_eq!(w(9, ImagePlacement::Page), Some(10));
        assert_eq!(w(55, ImagePlacement::Inline), Some(55));
        assert_eq!(w(255, ImagePlacement::Bleed), Some(100));
        assert_eq!(w(0, ImagePlacement::Beside), Some(25));
        assert_eq!(w(40, ImagePlacement::Beside), Some(40));
        assert_eq!(w(100, ImagePlacement::Beside), Some(50));
        assert_eq!(Block::image("a", "", None).image_width(), None);
        assert_eq!(Block::scene_break().image_width(), None);
    }

    fn alice() -> Block {
        // Lewis Carroll, Alice's Adventures in Wonderland (Gutenberg #11).
        Block::verse(vec![
            vec![Inline::text("“How doth the little crocodile")],
            vec![Inline::text("Improve his shining tail,")],
            vec![],
            vec![Inline::text("And pour the waters of the Nile")],
        ])
    }

    #[test]
    fn verse_indents_are_not_written_when_there_are_none() {
        let json = serde_json::to_value(alice()).unwrap();
        assert!(json.get("indents").is_none());
        // All zeros is none.
        let flush = alice().with_indents(vec![0, 0, 0, 0]);
        assert_eq!(flush, alice());
        let b = alice().with_indents(vec![0, 1, 0, 0]);
        let json = serde_json::to_value(&b).unwrap();
        assert_eq!(json["indents"], serde_json::json!([0, 1, 0, 0]));
        assert_eq!(serde_json::from_value::<Block>(json).unwrap(), b);
        // A block written before indents existed reads as flush.
        let old = serde_json::to_value(alice()).unwrap();
        let b: Block = serde_json::from_value(old).unwrap();
        assert_eq!(b.verse_indent(1), 0);
    }

    #[test]
    fn verse_indent_never_panics_on_a_damaged_file() {
        let b = alice().with_indents(vec![0, 1, 0, 2, 3, 4]);
        // The lines have levels; the two beyond the last line are ignored.
        assert_eq!(
            (0..8).map(|i| b.verse_indent(i)).collect::<Vec<_>>(),
            [0, 1, 0, 2, 0, 0, 0, 0]
        );
        // Fewer levels than lines: the missing ones are 0.
        let short = alice().with_indents(vec![0, 1]);
        assert_eq!(
            (0..5).map(|i| short.verse_indent(i)).collect::<Vec<_>>(),
            [0, 1, 0, 0, 0]
        );
        // Capped on read, kept as stored.
        let deep = alice().with_indents(vec![0, 200]);
        assert_eq!(deep.verse_indent(1), MAX_VERSE_INDENT);
        assert!(matches!(&deep, Block::Verse { indents, .. } if indents[1] == 200));
        // Not verse: 0.
        assert_eq!(Block::scene_break().verse_indent(0), 0);
        assert_eq!(
            Block::scene_break().with_indents(vec![1]).verse_indent(0),
            0
        );
    }

    #[test]
    fn picture_builders_ignore_other_blocks() {
        let b = Block::scene_break()
            .with_width(50)
            .with_placement(ImagePlacement::Page)
            .with_side(ImageSide::Left);
        assert_eq!(b, Block::scene_break());
    }

    fn sample() -> Book {
        let mut b = Book::new(Metadata {
            title: "Probe".into(),
            authors: vec!["A. Autorin".into()],
            lang: Lang("de-AT".into()),
            ..Default::default()
        });
        b.sections.push(Section::chapter(
            "Anfang",
            vec![
                Block::paragraph(vec![
                    Inline::text("Noch standen treu die "),
                    Inline::Emphasis {
                        content: vec![Inline::text("Pferde")],
                    },
                ]),
                Block::scene_break(),
            ],
        ));
        b
    }

    #[test]
    fn serde_round_trip() {
        let b = sample();
        let json = serde_json::to_string_pretty(&b).unwrap();
        let back: Book = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);
        assert!(json.contains("\"type\": \"scene-break\""));
    }

    #[test]
    fn imprint_is_left_out_until_set() {
        let mut b = sample();
        assert!(!serde_json::to_string(&b).unwrap().contains("imprint"));
        b.metadata.imprint.credits.push(Credit {
            role: "Lektorat".into(),
            name: "C. Lektorin".into(),
        });
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains(r#""imprint":{"credits":[{"role":"Lektorat","#));
        assert_eq!(serde_json::from_str::<Book>(&json).unwrap(), b);
    }

    #[test]
    fn block_refs() {
        let mut b = sample();
        let at = BlockRef {
            section: 0,
            block: 1,
        };
        assert_eq!(b.block(at), Some(&Block::scene_break()));
        *b.block_mut(at).unwrap() = Block::page_break();
        assert_eq!(b.sections[0].blocks[1], Block::page_break());
        assert_eq!(b.block(BlockRef { block: 2, ..at }), None);
        assert_eq!(b.block(BlockRef { section: 1, ..at }), None);
    }

    #[test]
    fn lang_primary() {
        assert_eq!(Lang("de-AT".into()).primary(), "de");
        assert_eq!(Lang("EN_gb".into()).primary(), "en");
    }

    #[test]
    fn lang_tags() {
        let tag = |s: &str| Lang(s.into()).tag();
        for (lang, written) in [
            ("en", "en"),
            ("de-AT", "de-AT"),
            (" en_GB ", "en-GB"),
            ("sr-Latn", "sr-Latn"),
            ("gsw", "gsw"),
        ] {
            assert_eq!(tag(lang).as_deref(), Some(written), "{lang}");
        }
        for lang in [
            "",
            "english",
            "e",
            "de--AT",
            "d3",
            "de AT",
            "de-AT-x-toolongsubtag",
        ] {
            assert_eq!(tag(lang), None, "{lang}");
        }
        assert_eq!(Lang("Deutsch".into()).effective(), Lang("en".into()));
        assert_eq!(Lang(" de_AT".into()).effective(), Lang("de-AT".into()));
    }

    fn numbered(sections: &[(SectionKind, Numbering)]) -> Vec<Option<(u32, u32)>> {
        let mut b = Book::new(Metadata::default());
        for (kind, numbering) in sections {
            b.sections.push(Section {
                kind: *kind,
                numbering: numbering.clone(),
                ..Section::chapter("", Vec::new())
            });
        }
        b.section_numbers()
            .into_iter()
            .map(|n| n.map(|n| (n.number, n.expected)))
            .collect()
    }

    #[test]
    fn chapters_and_parts_are_counted_apart() {
        use Numbering::{Auto, Explicit};
        use SectionKind::*;
        let numbers = numbered(&[
            (Prologue, Auto),
            (Part, Auto),
            (Chapter, Auto),
            (Chapter, Numbering::None),
            (Chapter, Explicit(5)),
            (Part, Auto),
            (Chapter, Auto),
            (Chapter, Explicit(2)),
            (Epilogue, Explicit(9)),
            (Part, Explicit(1)),
            (Chapter, Auto),
        ]);
        assert_eq!(
            numbers,
            [
                None,
                Some((1, 1)),
                Some((1, 1)),
                None,
                Some((5, 2)),
                Some((2, 2)),
                Some((6, 6)),
                Some((2, 7)),
                None,
                Some((1, 3)),
                Some((3, 3)),
            ]
        );
    }

    #[test]
    fn counting_saturates() {
        use Numbering::{Auto, Explicit};
        let numbers = numbered(&[
            (SectionKind::Chapter, Explicit(u32::MAX)),
            (SectionKind::Chapter, Auto),
            (SectionKind::Chapter, Explicit(4)),
            (SectionKind::Part, Explicit(u32::MAX)),
            (SectionKind::Part, Auto),
        ]);
        assert_eq!(
            numbers,
            [
                Some((u32::MAX, 1)),
                Some((u32::MAX, u32::MAX)),
                Some((4, u32::MAX)),
                Some((u32::MAX, 1)),
                Some((u32::MAX, u32::MAX)),
            ]
        );
    }

    #[test]
    fn plain_text_flattens() {
        let b = sample();
        let Block::Paragraph { content, .. } = &b.sections[0].blocks[0] else {
            panic!()
        };
        assert_eq!(Inline::plain_text(content), "Noch standen treu die Pferde");
    }
}
