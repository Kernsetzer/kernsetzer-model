//! The table block: rows and columns, header rows, per-column
//! alignment, merged cells and a caption. A cell holds inlines.
//!
//! A stored table can be damaged (a hand-edited file, a bug), so nothing
//! that draws a table reads `rows` by position: [`Table::layout`] turns any
//! stored table into the one grid every renderer agrees on, and
//! [`Table::normalized`] writes that grid back as a stored table. Reading a
//! file never normalizes it.

use crate::book::{Extra, Inline};
use serde::{Deserialize, Serialize};

/// The widest table [`Table::layout`] makes when `columns` is empty and the
/// width has to be taken from the rows.
pub const MAX_AUTO_COLUMNS: usize = 64;

/// A table: `{ "type": "table", "columns": …, "rows": … }` in `book.json`
/// (the block's JSON is flat; `Block::Table(Table)`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Table {
    /// One entry per column: how its text is aligned. Its length is the
    /// table's width.
    #[serde(default)]
    pub columns: Vec<TableColumn>,
    /// The first `header_rows` rows are headers (0: none). They repeat on
    /// every page in print and are `<thead>` in the EPUB.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub header_rows: u32,
    /// Row by row, only the cells that START in the row: a cell that a
    /// rowspan above covers is not listed again.
    #[serde(default)]
    pub rows: Vec<Vec<TableCell>>,
    /// Shown above the table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<Vec<Inline>>,
    /// Fields written by newer builds, kept on round-trip.
    #[serde(flatten)]
    pub extra: Extra,
}

/// One column of a [`Table`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TableColumn {
    #[serde(default)]
    pub align: ColumnAlign,
    /// Fields written by newer builds, kept on round-trip.
    #[serde(flatten)]
    pub extra: Extra,
}

/// One cell of a [`Table`]: inlines only (a cell of two paragraphs is two
/// lines of one cell, joined by `Inline::LineBreak`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    /// May be empty.
    #[serde(default)]
    pub content: Vec<Inline>,
    /// How many columns the cell covers (1, and then not written).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub colspan: u32,
    /// How many rows the cell covers (1, and then not written).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub rowspan: u32,
    /// Overrides the column's alignment for this cell: a heading centred over
    /// three right-aligned columns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<ColumnAlign>,
    /// Fields written by newer builds, kept on round-trip.
    #[serde(flatten)]
    pub extra: Extra,
}

/// How the text of a column (or one cell) is aligned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColumnAlign {
    #[default]
    Left,
    Center,
    Right,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn one() -> u32 {
    1
}

fn is_one(n: &u32) -> bool {
    *n == 1
}

impl TableColumn {
    pub fn new(align: ColumnAlign) -> Self {
        Self {
            align,
            extra: Extra::new(),
        }
    }
}

impl TableCell {
    pub fn new(content: Vec<Inline>) -> Self {
        Self {
            content,
            colspan: 1,
            rowspan: 1,
            align: None,
            extra: Extra::new(),
        }
    }

    /// A cell of plain text.
    pub fn text(s: impl Into<String>) -> Self {
        Self::new(vec![Inline::text(s)])
    }

    /// The cell covers `colspan` columns and `rowspan` rows.
    pub fn with_span(mut self, colspan: u32, rowspan: u32) -> Self {
        self.colspan = colspan;
        self.rowspan = rowspan;
        self
    }

    pub fn with_align(mut self, align: ColumnAlign) -> Self {
        self.align = Some(align);
        self
    }
}

/// A cell of [`Table::layout`]: where it stands in the grid, what it covers.
/// It borrows its text from the table, so a footnote in it can be kept for as
/// long as the table lives (the EPUB writer collects notes by reference).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedCell<'t> {
    pub row: usize,
    pub column: usize,
    /// At least 1; never past the grid, never across the header boundary.
    pub rowspan: usize,
    pub colspan: usize,
    /// The cell's own alignment, or else its first column's.
    pub align: ColumnAlign,
    /// The text, in parts: the stored cell's own content first (maybe empty),
    /// then the content of each stored cell of its row that had no room (rule
    /// 3), non-empty ones only. The parts are one cell, one under the other,
    /// a line break between them: [`PlacedCell::content`] joins them. Empty
    /// for a cell the layout made up.
    pub parts: Vec<&'t [Inline]>,
    /// `(row, index in the row's list)` of the stored cell; `None` for an
    /// empty cell the layout made up for a position nothing covered.
    pub source: Option<(usize, usize)>,
}

/// The table the model means: a complete grid. Every position
/// is covered exactly once, every row has a cell of its own, no span crosses
/// the table's edge or the header boundary. Cells are in reading order (by
/// row, then column).
#[derive(Debug, Clone, PartialEq)]
pub struct TableLayout<'t> {
    /// Width of the grid.
    pub columns: usize,
    /// Height of the grid (the stored rows).
    pub rows: usize,
    /// The leading rows that are headers, at most `rows`.
    pub header_rows: usize,
    /// The alignment of each column, `columns` of them.
    pub aligns: Vec<ColumnAlign>,
    pub cells: Vec<PlacedCell<'t>>,
}

impl<'t> PlacedCell<'t> {
    /// The cell's text as one run of inlines, the parts joined by line
    /// breaks (cloned; use `parts` to keep borrowing). A part that shows
    /// nothing (the stored cell's own, when it was empty) adds no line break.
    pub fn content(&self) -> Vec<Inline> {
        let mut out = Vec::new();
        for part in self.shown_parts() {
            if !out.is_empty() {
                out.push(Inline::LineBreak);
            }
            out.extend(part.iter().cloned());
        }
        out
    }

    /// The parts that show something (text or a footnote): what a writer
    /// joins with line breaks, so that none starts or ends the cell.
    pub fn shown_parts(&self) -> impl Iterator<Item = &'t [Inline]> + '_ {
        self.parts
            .iter()
            .copied()
            .filter(|p| !Inline::plain_text(p).trim().is_empty() || has_note(p))
    }

    /// Whether the cell shows nothing: no text and no footnote.
    pub fn is_blank(&self) -> bool {
        self.parts
            .iter()
            .all(|p| Inline::plain_text(p).trim().is_empty() && !has_note(p))
    }
}

impl Table {
    /// A table, normalized (see [`Table::normalized`]): what the importers
    /// build. `columns` may be empty, then the width is the widest row's.
    pub fn new(columns: Vec<TableColumn>, header_rows: u32, rows: Vec<Vec<TableCell>>) -> Self {
        Table {
            columns,
            header_rows,
            rows,
            caption: None,
            extra: Extra::new(),
        }
        .normalized()
    }

    /// The same table with a caption shown above it.
    pub fn with_caption(mut self, caption: Vec<Inline>) -> Self {
        self.caption = Some(caption);
        self
    }

    /// The width of the grid: `columns.len()`, or with no columns the widest
    /// row (the sum of its cells' spans), at most [`MAX_AUTO_COLUMNS`] and at
    /// least one when there is a row.
    fn width(&self) -> usize {
        if !self.columns.is_empty() {
            return self.columns.len();
        }
        let widest = self
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .fold(0usize, |sum, c| sum.saturating_add(span_of(c.colspan)))
            })
            .max()
            .unwrap_or(0);
        if self.rows.is_empty() {
            0
        } else {
            widest.clamp(1, MAX_AUTO_COLUMNS)
        }
    }

    /// The grid this table means, by the seven rules below. Total:
    /// no stored table panics or loses a cell's text.
    ///
    /// 1. The width is `columns.len()` (or the widest row, see [`Self::width`]).
    /// 2. A span of 0 is 1; a `colspan` is cut at the width and at a position
    ///    a rowspan above covers.
    /// 3. Cells are placed row by row, each at the first position at or after
    ///    the end of the previous one that no rowspan above covers. A cell
    ///    that does not fit has its text appended, after a line break, to the
    ///    last cell placed in its row.
    /// 4. A `rowspan` is cut at the last row and at the header boundary (a
    ///    cell that starts in a header row covers header rows only). A
    ///    rectangle cannot meet a position already taken: placement goes row
    ///    by row, so positions below a cell are free until a later row takes
    ///    them, and that row avoids the covered ones.
    /// 5. A position nothing covers and no cell was placed on becomes an empty
    ///    cell.
    /// 6. A row whose every position is covered by rowspans from above cuts
    ///    those spans to end above it, and is then placed like any row (its
    ///    listed cells take the freed positions, the rest become empty). The
    ///    ADR drops the text of such a row's listed cells; this keeps it,
    ///    since the rule only exists so that no row is without a cell.
    /// 7. `header_rows` is cut to the number of rows.
    pub fn layout(&self) -> TableLayout<'_> {
        let rows = self.rows.len();
        let width = self.width();
        let header = (self.header_rows as usize).min(rows);
        let aligns: Vec<ColumnAlign> = (0..width)
            .map(|c| self.columns.get(c).map_or(ColumnAlign::Left, |c| c.align))
            .collect();
        let mut placed: Vec<PlacedCell> = Vec::new();
        // `covered[c]`: the first row below the rowspan that covers column
        // `c`; the column is covered in row `r` while `r < covered[c]`.
        let mut covered = vec![0usize; width];
        // The placed cell that covers column `c` as far down as `covered[c]`.
        let mut owner: Vec<usize> = vec![0; width];

        for (r, listed) in self.rows.iter().enumerate() {
            // Rule 6: no position left for the row's own cells.
            if covered.iter().all(|&below| below > r) {
                let mut cut: Vec<usize> = owner.clone();
                cut.sort_unstable();
                cut.dedup();
                for i in cut {
                    let cell = &mut placed[i];
                    cell.rowspan = r - cell.row;
                    covered[cell.column..cell.column + cell.colspan].fill(r);
                }
            }
            let mut next = 0;
            let mut last: Option<usize> = None;
            for (index, cell) in listed.iter().enumerate() {
                let mut column = next;
                while column < width && covered[column] > r {
                    column += 1;
                }
                if column >= width {
                    // Rule 3: no room. A position was free for the row's first
                    // cell, so there is a cell placed in the row to take it.
                    if let Some(to) = last
                        && !cell.content.is_empty()
                    {
                        placed[to].parts.push(&cell.content);
                    }
                    continue;
                }
                let mut colspan = 1;
                while colspan < span_of(cell.colspan)
                    && column + colspan < width
                    && covered[column + colspan] <= r
                {
                    colspan += 1;
                }
                let mut rowspan = span_of(cell.rowspan).min(rows - r);
                if r < header {
                    rowspan = rowspan.min(header - r);
                }
                let at = placed.len();
                for c in column..column + colspan {
                    covered[c] = r + rowspan;
                    owner[c] = at;
                }
                placed.push(PlacedCell {
                    row: r,
                    column,
                    rowspan,
                    colspan,
                    align: cell.align.unwrap_or(aligns[column]),
                    parts: vec![&cell.content],
                    source: Some((r, index)),
                });
                next = column + colspan;
                last = Some(at);
            }
            // Rule 5: positions nothing covers.
            for column in 0..width {
                if covered[column] <= r {
                    covered[column] = r + 1;
                    owner[column] = placed.len();
                    placed.push(PlacedCell {
                        row: r,
                        column,
                        rowspan: 1,
                        colspan: 1,
                        align: aligns[column],
                        parts: Vec::new(),
                        source: None,
                    });
                }
            }
        }
        placed.sort_by_key(|c| (c.row, c.column));
        TableLayout {
            columns: width,
            rows,
            header_rows: header,
            aligns,
            cells: placed,
        }
    }

    /// The layout written back as a stored table, in the form importers and
    /// [`Table::new`] make: one column entry per position, only the cells that
    /// start in each row, spans cut, gaps filled with empty cells,
    /// `header_rows` cut. Its layout is this table's, cell for cell (the
    /// `source` of a cell made up now points at its stored place).
    pub fn normalized(&self) -> Table {
        let layout = self.layout();
        let mut columns = self.columns.clone();
        columns.resize_with(layout.columns, TableColumn::default);
        let mut rows: Vec<Vec<TableCell>> = vec![Vec::new(); layout.rows];
        for cell in layout.cells {
            let mut stored = cell
                .source
                .and_then(|(r, i)| self.rows.get(r)?.get(i).cloned())
                .unwrap_or_else(|| TableCell::new(Vec::new()));
            stored.content = cell.content();
            stored.colspan = u32::try_from(cell.colspan).unwrap_or(u32::MAX);
            stored.rowspan = u32::try_from(cell.rowspan).unwrap_or(u32::MAX);
            if let Some(row) = rows.get_mut(cell.row) {
                row.push(stored);
            }
        }
        Table {
            columns,
            header_rows: u32::try_from(layout.header_rows).unwrap_or(u32::MAX),
            rows,
            caption: self.caption.clone(),
            extra: self.extra.clone(),
        }
    }
}

/// A span as a count: 0 (a damaged file) is 1.
fn span_of(n: u32) -> usize {
    (n as usize).max(1)
}

/// Whether `inlines` hold a footnote, which `plain_text` leaves out.
fn has_note(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::Footnote { .. } => true,
        Inline::Emphasis { content }
        | Inline::Strong { content }
        | Inline::SmallCaps { content }
        | Inline::Superscript { content }
        | Inline::Subscript { content }
        | Inline::Link { content, .. } => has_note(content),
        Inline::Text { .. } | Inline::LineBreak => false,
    })
}

#[cfg(test)]
mod tests;
