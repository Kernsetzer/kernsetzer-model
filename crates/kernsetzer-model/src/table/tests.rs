//! Unit tests of the table block: the flat serde shape and one test per rule
//! of `Table::layout`. The text is Austen's *Pride and Prejudice*.

use super::*;
use crate::book::Block;
use serde_json::{Value, json};

fn c(s: &str) -> TableCell {
    TableCell::text(s)
}

fn span(s: &str, colspan: u32, rowspan: u32) -> TableCell {
    TableCell::text(s).with_span(colspan, rowspan)
}

fn cols(aligns: &[ColumnAlign]) -> Vec<TableColumn> {
    aligns.iter().map(|&a| TableColumn::new(a)).collect()
}

fn left(n: usize) -> Vec<TableColumn> {
    cols(&vec![ColumnAlign::Left; n])
}

/// A stored table as a damaged file might hold it: not normalized.
fn table(columns: Vec<TableColumn>, header_rows: u32, rows: Vec<Vec<TableCell>>) -> Table {
    Table {
        columns,
        header_rows,
        rows,
        caption: None,
        extra: Extra::new(),
    }
}

/// The layout as `(row, column, rowspan, colspan, text)`.
fn grid(t: &Table) -> Vec<(usize, usize, usize, usize, String)> {
    t.layout()
        .cells
        .iter()
        .map(|c| {
            (
                c.row,
                c.column,
                c.rowspan,
                c.colspan,
                Inline::plain_text(&c.content()),
            )
        })
        .collect()
}

fn g(
    row: usize,
    column: usize,
    rowspan: usize,
    colspan: usize,
    text: &str,
) -> (usize, usize, usize, usize, String) {
    (row, column, rowspan, colspan, text.to_string())
}

/// Every position is covered exactly once, every row has a cell of its own,
/// and no span crosses the edge or the header boundary.
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
}

// ----- serde: the flat shape ------------------------------------------------

/// The JSON of a table with unknown fields at the four levels (block,
/// column, cell, and a cell's footnote paragraph is an ordinary `Block`).
fn json_table() -> Value {
    json!({
        "type": "table",
        "columns": [
            { "align": "left", "x-width": 12.5 },
            { "align": "right" },
            { "align": "center" }
        ],
        "header_rows": 1,
        "rows": [
            [
                { "content": [ { "type": "text", "text": "Jane" } ], "x-cell": { "type": "odd" } },
                { "content": [ { "type": "text", "text": "the eldest Miss Bennet" } ],
                  "colspan": 2, "align": "center" }
            ],
            [
                { "content": [ { "type": "text", "text": "Jane" } ] },
                { "content": [] },
                { "content": [ { "type": "text", "text": "the eldest" },
                               { "type": "footnote", "blocks": [
                                 { "type": "paragraph", "content": [], "x-note": 1 } ] } ],
                  "rowspan": 1 }
            ]
        ],
        "caption": [ { "type": "text", "text": "Longbourn" } ],
        "x-block": [1, "two", null]
    })
}

#[test]
fn block_table_is_flat_and_keeps_unknown_fields_at_four_levels() {
    let json = json_table();
    let block: Block = serde_json::from_value(json.clone()).unwrap();
    let Block::Table(t) = &block else {
        panic!("not a table: {block:?}");
    };
    assert_eq!(t.columns.len(), 3);
    assert_eq!(t.columns[1].align, ColumnAlign::Right);
    assert_eq!(t.header_rows, 1);
    assert_eq!(t.rows[0][1].colspan, 2);
    assert_eq!(t.rows[0][1].align, Some(ColumnAlign::Center));
    // Unknown fields: the table, a column, a cell, a footnote's block.
    assert_eq!(t.extra["x-block"], json!([1, "two", null]));
    assert_eq!(t.columns[0].extra["x-width"], 12.5);
    assert_eq!(t.rows[0][0].extra["x-cell"], json!({"type": "odd"}));
    let Inline::Footnote { blocks } = &t.rows[1][2].content[1] else {
        panic!("{:?}", t.rows[1][2]);
    };
    assert_eq!(blocks[0].extra()["x-note"], 1);
    // `Block::extra` reaches the table's own; the tag is not an unknown field.
    assert_eq!(block.extra()["x-block"], json!([1, "two", null]));
    assert!(!block.extra().contains_key("type"));
    // And back: the written JSON is the read JSON, minus the explicit
    // `"rowspan": 1` (a default is not written).
    let mut written = serde_json::to_value(&block).unwrap();
    assert_eq!(written["rows"][1][2].get("rowspan"), None);
    written["rows"][1][2]["rowspan"] = json!(1);
    assert_eq!(written, json);
    assert_eq!(serde_json::from_value::<Block>(written).unwrap(), block);
}

#[test]
fn defaults_are_not_written_and_missing_fields_read_as_defaults() {
    let plain = Table::new(left(2), 0, vec![vec![c("Jane"), c("Elizabeth")]]);
    let json = serde_json::to_value(Block::table(plain)).unwrap();
    assert_eq!(
        json,
        json!({
            "type": "table",
            "columns": [ { "align": "left" }, { "align": "left" } ],
            "rows": [[
                { "content": [ { "type": "text", "text": "Jane" } ] },
                { "content": [ { "type": "text", "text": "Elizabeth" } ] }
            ]]
        })
    );
    // Spans, a header count, a caption and a cell's alignment, when set.
    let spanned = Table::new(
        left(2),
        1,
        vec![vec![span("Mary", 2, 1).with_align(ColumnAlign::Right)]],
    )
    .with_caption(vec![Inline::text("Longbourn")]);
    let json = serde_json::to_value(Block::table(spanned)).unwrap();
    assert_eq!(json["header_rows"], 1);
    assert_eq!(json["rows"][0][0]["colspan"], 2);
    assert_eq!(json["rows"][0][0]["align"], "right");
    assert!(json["rows"][0][0].get("rowspan").is_none());
    assert_eq!(json["caption"][0]["text"], "Longbourn");
    // A table written with nothing but its tag reads as an empty one.
    let bare: Block = serde_json::from_value(json!({"type": "table"})).unwrap();
    assert_eq!(bare, Block::table(Table::default()));
    // A cell with nothing but a text, and a column with nothing.
    let t: Table = serde_json::from_value(json!({
        "columns": [{}],
        "rows": [[{ "content": [ { "type": "text", "text": "Lydia" } ] }]]
    }))
    .unwrap();
    assert_eq!(t.columns[0].align, ColumnAlign::Left);
    assert_eq!((t.rows[0][0].colspan, t.rows[0][0].rowspan), (1, 1));
}

#[test]
fn damaged_values_are_kept_as_given() {
    // Spans of 0 and beyond any sensible size, an empty `columns`: all read,
    // all written back unchanged (only drawing cuts them).
    let json = json!({
        "type": "table",
        "columns": [],
        "header_rows": 4000000000u32,
        "rows": [[
            { "content": [], "colspan": 0, "rowspan": 4294967295u32 },
            { "content": [] }
        ]]
    });
    let block: Block = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(serde_json::to_value(&block).unwrap(), json);
}

#[test]
fn column_align_is_kebab_case() {
    for (a, s) in [
        (ColumnAlign::Left, "left"),
        (ColumnAlign::Center, "center"),
        (ColumnAlign::Right, "right"),
    ] {
        assert_eq!(serde_json::to_value(a).unwrap(), s);
        assert_eq!(serde_json::from_value::<ColumnAlign>(json!(s)).unwrap(), a);
    }
    assert!(serde_json::from_value::<ColumnAlign>(json!("decimal")).is_err());
}

// ----- layout: one test per rule --------------------------------------------

#[test]
fn rule_1_width_is_the_number_of_columns() {
    let t = table(left(3), 0, vec![vec![c("Jane"), c("Elizabeth")]]);
    let l = t.layout();
    assert_eq!((l.columns, l.rows), (3, 1));
    // The short row gets an empty cell (rule 5).
    assert_eq!(
        grid(&t),
        [
            g(0, 0, 1, 1, "Jane"),
            g(0, 1, 1, 1, "Elizabeth"),
            g(0, 2, 1, 1, "")
        ]
    );
    assert_complete(&l);
}

#[test]
fn rule_1_without_columns_the_width_is_the_widest_row() {
    let t = table(
        vec![],
        0,
        vec![
            vec![c("Jane"), c("Elizabeth")],
            vec![span("Mary", 2, 1), c("Catherine")],
        ],
    );
    let l = t.layout();
    assert_eq!(l.columns, 3);
    assert_eq!(l.aligns, vec![ColumnAlign::Left; 3]);
    assert_eq!(
        grid(&t),
        [
            g(0, 0, 1, 1, "Jane"),
            g(0, 1, 1, 1, "Elizabeth"),
            g(0, 2, 1, 1, ""),
            g(1, 0, 1, 2, "Mary"),
            g(1, 2, 1, 1, "Catherine"),
        ]
    );
    assert_complete(&l);
}

#[test]
fn rule_1_the_width_from_rows_is_at_most_64_and_no_text_is_lost() {
    let names: Vec<String> = (0..70).map(|i| format!("Lydia {i}")).collect();
    let t = table(vec![], 0, vec![names.iter().map(|n| c(n)).collect()]);
    let l = t.layout();
    assert_eq!(l.columns, MAX_AUTO_COLUMNS);
    assert_complete(&l);
    // The six that did not fit stand after the 64th, each on a line.
    let last = l.cells.last().unwrap();
    assert_eq!(last.column, 63);
    assert_eq!(
        Inline::plain_text(&last.content()),
        "Lydia 63 Lydia 64 Lydia 65 Lydia 66 Lydia 67 Lydia 68 Lydia 69"
    );
    assert_eq!(
        last.content()
            .iter()
            .filter(|i| matches!(i, Inline::LineBreak))
            .count(),
        6
    );
}

#[test]
fn rule_1_empty_tables() {
    let empty = Table::default();
    let none = empty.layout();
    assert_eq!((none.columns, none.rows, none.cells.len()), (0, 0, 0));
    // Columns and no rows: a grid with no rows.
    let no_rows = table(left(2), 1, vec![]);
    let l = no_rows.layout();
    assert_eq!(
        (l.columns, l.rows, l.header_rows, l.cells.len()),
        (2, 0, 0, 0)
    );
    // No columns and an empty row: one position, one empty cell.
    let t = table(vec![], 0, vec![vec![]]);
    assert_eq!(grid(&t), [g(0, 0, 1, 1, "")]);
}

#[test]
fn rule_2_spans_of_zero_become_one() {
    let t = table(
        left(2),
        0,
        vec![vec![span("Jane", 0, 0), span("Mary", 0, 0)]],
    );
    assert_eq!(grid(&t), [g(0, 0, 1, 1, "Jane"), g(0, 1, 1, 1, "Mary")]);
}

#[test]
fn rule_2_a_colspan_is_cut_at_the_edge() {
    let t = table(left(3), 0, vec![vec![c("Jane"), span("Elizabeth", 5, 1)]]);
    assert_eq!(
        grid(&t),
        [g(0, 0, 1, 1, "Jane"), g(0, 1, 1, 2, "Elizabeth")]
    );
    // And u32::MAX does not overflow anything.
    let t = table(left(2), 0, vec![vec![span("Mary", u32::MAX, u32::MAX)]]);
    assert_eq!(grid(&t), [g(0, 0, 1, 2, "Mary")]);
}

#[test]
fn rule_2_a_colspan_is_cut_where_a_rowspan_above_covers() {
    let t = table(
        left(3),
        0,
        vec![
            vec![c("Jane"), c("Elizabeth"), span("Mary", 1, 2)],
            vec![span("Catherine", 3, 1)],
        ],
    );
    assert_eq!(
        grid(&t),
        [
            g(0, 0, 1, 1, "Jane"),
            g(0, 1, 1, 1, "Elizabeth"),
            g(0, 2, 2, 1, "Mary"),
            g(1, 0, 1, 2, "Catherine"),
        ]
    );
    assert_complete(&t.layout());
}

#[test]
fn rule_3_cells_skip_the_positions_a_rowspan_covers() {
    let t = table(
        left(3),
        0,
        vec![
            vec![c("Jane"), span("Elizabeth", 1, 2), c("Mary")],
            vec![c("Catherine"), c("Lydia")],
        ],
    );
    assert_eq!(
        grid(&t),
        [
            g(0, 0, 1, 1, "Jane"),
            g(0, 1, 2, 1, "Elizabeth"),
            g(0, 2, 1, 1, "Mary"),
            g(1, 0, 1, 1, "Catherine"),
            g(1, 2, 1, 1, "Lydia"),
        ]
    );
    // Sources point at the stored cells.
    let sources: Vec<_> = t.layout().cells.iter().map(|c| c.source).collect();
    assert_eq!(
        sources,
        [
            Some((0, 0)),
            Some((0, 1)),
            Some((0, 2)),
            Some((1, 0)),
            Some((1, 1))
        ]
    );
}

#[test]
fn rule_3_a_cell_that_does_not_fit_joins_the_last_one_after_a_line_break() {
    let t = table(
        left(2),
        0,
        vec![vec![
            c("Jane"),
            c("Elizabeth"),
            c("Mary"),
            TableCell::new(vec![]),
        ]],
    );
    let l = t.layout();
    assert_eq!(l.cells.len(), 2);
    assert_eq!(
        l.cells[1].content(),
        [
            Inline::text("Elizabeth"),
            Inline::LineBreak,
            Inline::text("Mary")
        ]
    );
    // An empty surplus cell adds neither text nor a line break.
    assert_eq!(l.cells[0].content(), [Inline::text("Jane")]);
    assert_eq!(l.cells[1].parts.len(), 2);
}

#[test]
fn rule_3_an_empty_absorbing_cell_starts_with_no_line_break() {
    let t = table(left(1), 0, vec![vec![c(""), c("Jane"), c("Lydia")]]);
    let l = t.layout();
    assert_eq!(l.cells.len(), 1);
    assert_eq!(
        l.cells[0].content(),
        [
            Inline::text("Jane"),
            Inline::LineBreak,
            Inline::text("Lydia")
        ]
    );
    // The normal form stores that, not a leading line break.
    let n = t.normalized();
    assert_eq!(n.rows[0][0].content[0], Inline::text("Jane"));
}

#[test]
fn rule_3_a_surplus_cell_after_a_covered_position_joins_the_row_s_last_cell() {
    // The row has room for one cell (the other position is covered).
    let t = table(
        left(2),
        0,
        vec![
            vec![c("Jane"), span("Elizabeth", 1, 2)],
            vec![c("Mary"), c("Catherine")],
        ],
    );
    assert_eq!(
        grid(&t).iter().map(|c| c.4.as_str()).collect::<Vec<_>>(),
        ["Jane", "Elizabeth", "Mary Catherine"]
    );
}

#[test]
fn rule_4_a_rowspan_is_cut_at_the_last_row() {
    let t = table(
        left(2),
        0,
        vec![vec![span("Jane", 1, 9), c("Mary")], vec![c("Lydia")]],
    );
    assert_eq!(
        grid(&t),
        [
            g(0, 0, 2, 1, "Jane"),
            g(0, 1, 1, 1, "Mary"),
            g(1, 1, 1, 1, "Lydia"),
        ]
    );
    assert_complete(&t.layout());
}

#[test]
fn rule_4_a_rowspan_is_cut_at_the_header_boundary() {
    // "Station" starts in the header and reaches three rows: it covers the
    // header's two.
    let t = table(
        left(2),
        2,
        vec![
            vec![span("Jane", 1, 3), span("Netherfield", 1, 1)],
            vec![c("Mr. Bingley")],
            vec![c("Mary")],
        ],
    );
    let l = t.layout();
    assert_eq!(l.header_rows, 2);
    assert_eq!(
        grid(&t),
        [
            g(0, 0, 2, 1, "Jane"),
            g(0, 1, 1, 1, "Netherfield"),
            g(1, 1, 1, 1, "Mr. Bingley"),
            g(2, 0, 1, 1, "Mary"),
            g(2, 1, 1, 1, ""),
        ]
    );
    assert_complete(&l);
}

#[test]
fn rule_4_a_body_rowspan_is_not_cut_by_the_header() {
    let t = table(
        left(2),
        1,
        vec![
            vec![c("Jane"), c("Longbourn")],
            vec![span("Jane", 1, 2), c("eldest")],
            vec![c("twenty")],
        ],
    );
    assert_eq!(grid(&t)[2], g(1, 0, 2, 1, "Jane"));
    assert_complete(&t.layout());
}

#[test]
fn rule_5_a_position_nothing_covers_becomes_an_empty_cell() {
    let t = table(
        left(3),
        0,
        vec![vec![], vec![span("Jane", 1, 2)], vec![c("Mary")]],
    );
    let l = t.layout();
    assert_eq!(
        grid(&t),
        [
            g(0, 0, 1, 1, ""),
            g(0, 1, 1, 1, ""),
            g(0, 2, 1, 1, ""),
            g(1, 0, 2, 1, "Jane"),
            g(1, 1, 1, 1, ""),
            g(1, 2, 1, 1, ""),
            g(2, 1, 1, 1, "Mary"),
            g(2, 2, 1, 1, ""),
        ]
    );
    assert!(l.cells.iter().filter(|c| c.source.is_none()).count() == 6);
    assert_complete(&l);
}

#[test]
fn rule_6_a_row_covered_from_above_cuts_the_spans_that_reach_it() {
    // Both columns are covered in row 1 by rowspans from row 0; row 1 lists
    // nothing. The spans end above it, and its positions become empty cells.
    let t = table(
        left(2),
        0,
        vec![
            vec![span("Jane", 1, 3), span("Mary", 1, 2)],
            vec![],
            vec![c("Lydia")],
        ],
    );
    assert_eq!(
        grid(&t),
        [
            g(0, 0, 1, 1, "Jane"),
            g(0, 1, 1, 1, "Mary"),
            g(1, 0, 1, 1, ""),
            g(1, 1, 1, 1, ""),
            g(2, 0, 1, 1, "Lydia"),
            g(2, 1, 1, 1, ""),
        ]
    );
    assert_complete(&t.layout());
}

#[test]
fn rule_6_the_cells_a_covered_row_lists_keep_their_text() {
    // The ADR drops these; the cut frees the positions and they are placed.
    let t = table(
        left(2),
        0,
        vec![
            vec![span("Jane", 1, 2), span("Mary", 1, 2)],
            vec![c("Catherine"), c("Lydia")],
        ],
    );
    assert_eq!(
        grid(&t),
        [
            g(0, 0, 1, 1, "Jane"),
            g(0, 1, 1, 1, "Mary"),
            g(1, 0, 1, 1, "Catherine"),
            g(1, 1, 1, 1, "Lydia"),
        ]
    );
}

#[test]
fn rule_6_a_wide_cell_is_cut_over_all_its_columns() {
    let t = table(
        left(3),
        0,
        vec![
            vec![span("Jane", 2, 4), span("Mary", 1, 2)],
            vec![],
            vec![c("Lydia")],
        ],
    );
    assert_eq!(
        grid(&t),
        [
            g(0, 0, 1, 2, "Jane"),
            g(0, 2, 1, 1, "Mary"),
            g(1, 0, 1, 1, ""),
            g(1, 1, 1, 1, ""),
            g(1, 2, 1, 1, ""),
            g(2, 0, 1, 1, "Lydia"),
            g(2, 1, 1, 1, ""),
            g(2, 2, 1, 1, ""),
        ]
    );
    assert_complete(&t.layout());
}

#[test]
fn rule_7_header_rows_is_cut_to_the_number_of_rows() {
    let t = table(left(1), 9, vec![vec![c("Jane")], vec![c("Mary")]]);
    assert_eq!(t.layout().header_rows, 2);
    assert_eq!(table(left(1), 9, vec![]).layout().header_rows, 0);
    // A header over every row: spans are cut at the table's end all the same.
    let t = table(left(1), 9, vec![vec![span("Jane", 1, 5)], vec![]]);
    assert_complete(&t.layout());
}

#[test]
fn alignment_is_the_cell_s_own_or_its_first_column_s() {
    let t = table(
        cols(&[ColumnAlign::Left, ColumnAlign::Right, ColumnAlign::Center]),
        0,
        vec![
            vec![c("Jane"), span("two", 2, 1)],
            vec![
                c("Mary").with_align(ColumnAlign::Center),
                span("and", 2, 1).with_align(ColumnAlign::Left),
            ],
            vec![span("twenty", 2, 1), c("years")],
        ],
    );
    let aligns: Vec<_> = t.layout().cells.iter().map(|c| c.align).collect();
    use ColumnAlign::*;
    assert_eq!(aligns, [Left, Right, Center, Left, Left, Center]);
}

#[test]
fn cells_come_in_reading_order() {
    let t = table(
        left(3),
        0,
        vec![vec![c("a"), span("b", 1, 2), c("c")], vec![c("d")]],
    );
    let order: Vec<_> = t.layout().cells.iter().map(|c| (c.row, c.column)).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted);
}

// ----- normalized, new, the fallback ---------------------------------------

#[test]
fn normalized_writes_the_layout_back_and_lays_out_as_itself() {
    let damaged = table(
        vec![],
        7,
        vec![
            vec![span("Jane", 0, 9), c("Elizabeth"), c("Mary")],
            vec![],
            vec![c("Catherine").with_align(ColumnAlign::Right)],
        ],
    );
    let n = damaged.normalized();
    assert_eq!(n.columns.len(), 3);
    assert_eq!(n.header_rows, 3);
    assert_eq!(n.rows.len(), 3);
    // Only the cells that start in a row, spans cut, gaps filled.
    assert_eq!(n.rows[0].len(), 3);
    assert_eq!(n.rows[1].len(), 2);
    assert_eq!(n.rows[2].len(), 2);
    assert_eq!(n.rows[0][0].rowspan, 3);
    assert_eq!(n.rows[0][0].colspan, 1);
    // The cell keeps its own alignment, and stands where Jane's rowspan left room.
    assert_eq!(n.rows[2][0].align, Some(ColumnAlign::Right));
    assert_eq!(n.layout().cells[5].column, 1);
    // Same grid, and the normal form is a fixed point.
    let (a, b) = (damaged.layout(), n.layout());
    assert_eq!(a.columns, b.columns);
    assert_eq!(a.header_rows, b.header_rows);
    let strip = |l: &TableLayout| {
        l.cells
            .iter()
            .map(|c| (c.row, c.column, c.rowspan, c.colspan, c.align, c.content()))
            .collect::<Vec<_>>()
    };
    assert_eq!(strip(&a), strip(&b));
    assert!(b.cells.iter().all(|c| c.source.is_some()));
    assert_eq!(n.normalized(), n);
    assert_complete(&b);
}

#[test]
fn new_normalizes_and_a_valid_table_is_unchanged() {
    let rows = vec![
        vec![span("Jane", 1, 2), span("the eldest Miss Bennet", 2, 1)],
        vec![c("Mr. Bingley"), c("Mr. Darcy")],
    ];
    let valid = Table::new(left(3), 2, rows.clone());
    assert_eq!(valid.rows, rows);
    assert_eq!(valid.header_rows, 2);
    // A short row is filled, columns appear for a table without any.
    let t = Table::new(
        vec![],
        0,
        vec![vec![c("Jane"), c("Mary")], vec![c("Lydia")]],
    );
    assert_eq!(t.columns.len(), 2);
    assert_eq!(t.rows[1].len(), 2);
}

#[test]
fn normalized_keeps_extra_and_the_caption() {
    let mut t = table(left(1), 0, vec![vec![c("Jane")]]);
    t.caption = Some(vec![Inline::text("Longbourn")]);
    t.extra.insert("x-table".into(), json!(1));
    t.columns[0].extra.insert("x-col".into(), json!(2));
    t.rows[0][0].extra.insert("x-cell".into(), json!(3));
    assert_eq!(t.normalized(), t);
}
