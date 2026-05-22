//! Stem AST → .docx via the `docx-rs` crate.
//!
//! MVP scope: headings (h1..h6 mapped to built-in Word styles
//! Heading1..6), paragraphs, ordered/unordered lists, blockquote,
//! fenced code blocks (Courier New), inline runs with italic/bold/
//! strikethrough.
//!
//! Out of scope: tables, images, links, footnotes, headers/footers,
//! page numbering, tracked changes, comments, theme/style customization.

use docx_rs::{
    AbstractNumbering, AlignmentType, BorderType, Docx, IndentLevel, Level, LevelJc, LevelText,
    NumberFormat, Numbering, NumberingId, Paragraph, Run, RunFonts, Shading, SpecialIndentType,
    Start, Table, TableBorder, TableBorders, TableCell, TableRow, VAlignType, VMergeType,
};
use stem_core::ast::{Block, Body, Document, TextPiece};
use stem_core::theme::Theme;
use stem_core::Exporter;
use thiserror::Error;

#[derive(Default)]
pub struct DocxExporter;

impl DocxExporter {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Error)]
pub enum DocxError {
    #[error("docx pack: {0}")]
    Pack(String),
}

impl Exporter for DocxExporter {
    type Output = Vec<u8>;
    type Error = DocxError;
    fn export(&self, doc: &Document, theme: &Theme) -> Result<Vec<u8>, DocxError> {
        let cooked = stem_parser::cook_document(doc);
        let mut out = Docx::new();
        // Register a single numbering definition for ordered lists (id 1)
        // and unordered (id 2). Each list-item paragraph references the
        // matching NumId. The cap of 9 nesting levels matches Word's
        // built-in expectation; we pre-create all 9 so deeply nested
        // lists don't crash.
        out = out
            .add_abstract_numbering(abstract_numbering(1, false))
            .add_numbering(Numbering::new(1, 1))
            .add_abstract_numbering(abstract_numbering(2, true))
            .add_numbering(Numbering::new(2, 2));

        for block in &cooked.blocks {
            out = emit_block(out, block, theme, 0);
        }

        let xmldocx = out.build();
        let mut buf: Vec<u8> = Vec::new();
        let cursor = std::io::Cursor::new(&mut buf);
        xmldocx
            .pack(cursor)
            .map_err(|e| DocxError::Pack(e.to_string()))?;
        Ok(buf)
    }
}

// --- emission -----------------------------------------------------------

fn emit_block(docx: Docx, b: &Block, theme: &Theme, _depth: usize) -> Docx {
    match b.name.as_str() {
        "h1" => docx.add_paragraph(heading_para(b, 1)),
        "h2" => docx.add_paragraph(heading_para(b, 2)),
        "h3" => docx.add_paragraph(heading_para(b, 3)),
        "h4" => docx.add_paragraph(heading_para(b, 4)),
        "h5" => docx.add_paragraph(heading_para(b, 5)),
        "h6" => docx.add_paragraph(heading_para(b, 6)),
        "p" => docx.add_paragraph(text_para(b, None)),
        "blockquote" => docx.add_paragraph(blockquote_para(b)),
        "code" => emit_code_block(docx, b),
        "ol" => emit_list(docx, b, true),
        "ul" => emit_list(docx, b, false),
        "hr" => docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text("───"))),
        "table" => emit_table(docx, b, theme),
        _ => docx.add_paragraph(text_para(b, None)),
    }
}

fn heading_para(b: &Block, level: u8) -> Paragraph {
    let style = format!("Heading{}", level);
    let mut p = Paragraph::new().style(&style);
    for run in collect_runs(b, RunStyle::default()) {
        p = p.add_run(run);
    }
    p
}

fn text_para(b: &Block, style: Option<&str>) -> Paragraph {
    let mut p = Paragraph::new();
    if let Some(s) = style {
        p = p.style(s);
    }
    for run in collect_runs(b, RunStyle::default()) {
        p = p.add_run(run);
    }
    p
}

fn blockquote_para(b: &Block) -> Paragraph {
    let mut style = RunStyle::default();
    style.italic = true;
    let mut p = Paragraph::new().indent(Some(720), None, None, None);
    for run in collect_runs(b, style) {
        p = p.add_run(run);
    }
    p
}

fn emit_code_block(docx: Docx, b: &Block) -> Docx {
    // One paragraph per line, each in monospace.
    let text = b.plain_text().unwrap_or_default();
    let mut docx = docx;
    for line in text.lines() {
        let run = Run::new()
            .fonts(RunFonts::new().ascii("Courier New").hi_ansi("Courier New"))
            .add_text(line);
        docx = docx.add_paragraph(Paragraph::new().add_run(run));
    }
    docx
}

fn emit_list(docx: Docx, b: &Block, ordered: bool) -> Docx {
    let num_id = if ordered { 1 } else { 2 };
    let mut docx = docx;
    if let Body::Children(items) = &b.body {
        for item in items {
            let mut p = Paragraph::new().numbering(NumberingId::new(num_id), IndentLevel::new(0));
            for run in collect_runs(item, RunStyle::default()) {
                p = p.add_run(run);
            }
            docx = docx.add_paragraph(p);
        }
    }
    docx
}

// --- inline runs --------------------------------------------------------

#[derive(Clone, Copy, Default)]
struct RunStyle {
    bold: bool,
    italic: bool,
    strike: bool,
    monospace: bool,
}

/// Walk a block's body and collect docx Runs with appropriate styling
/// based on inline `@text` elements. Whitespace and plain text inherit
/// `base`; `@text` styled spans override.
fn collect_runs(b: &Block, base: RunStyle) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    if let Body::Text(pieces) = &b.body {
        for piece in pieces {
            match piece {
                TextPiece::Literal { text, .. } => {
                    out.push(styled_run(text, base));
                }
                TextPiece::Inline(inline) => {
                    let style = inline_style(inline, base);
                    let text = inline.plain_text().unwrap_or_default();
                    if !text.is_empty() {
                        out.push(styled_run(&text, style));
                    }
                }
            }
        }
    }
    out
}

fn styled_run(text: &str, style: RunStyle) -> Run {
    let mut run = Run::new().add_text(text);
    if style.bold {
        run = run.bold();
    }
    if style.italic {
        run = run.italic();
    }
    if style.strike {
        run = run.strike();
    }
    if style.monospace {
        run = run.fonts(RunFonts::new().ascii("Courier New").hi_ansi("Courier New"));
    }
    run
}

fn inline_style(b: &Block, base: RunStyle) -> RunStyle {
    match b.name.as_str() {
        "text" => RunStyle {
            bold: base.bold || b.prop_str("weight") == Some("bold"),
            italic: base.italic || b.prop_str("style") == Some("italic"),
            strike: base.strike || b.prop_str("decoration") == Some("strike"),
            monospace: base.monospace,
        },
        "code" => RunStyle {
            monospace: true,
            ..base
        },
        _ => base,
    }
}

// --- tables -------------------------------------------------------------

/// Emit an optional caption paragraph (if `caption` prop set) followed by
/// a `<w:tbl>` representing the rows.
///
/// Supports: `border` (none|outer|all), `stripe`, `caption`, per-row
/// `kind` (header rows get bold runs + a light shading), per-cell
/// `colspan`, `rowspan`, `bg`, `align`, `valign`.
fn emit_table(docx: Docx, b: &Block, theme: &Theme) -> Docx {
    let border_mode = match b.prop_str("border").unwrap_or("none") {
        "all" => BorderMode::All,
        "outer" => BorderMode::Outer,
        _ => BorderMode::None,
    };
    let stripe = b.prop_str("stripe").map(|v| v == "true").unwrap_or(false);

    let mut docx = docx;
    if let Some(caption) = b.prop_str("caption") {
        // Word's built-in "Caption" paragraph style. If the style isn't
        // registered we still get plain centered italic text via the
        // theme; we don't define our own style here.
        let para = Paragraph::new()
            .style("Caption")
            .add_run(Run::new().add_text(caption));
        docx = docx.add_paragraph(para);
    }

    let rows = match &b.body {
        Body::Children(c) => c.as_slice(),
        _ => return docx,
    };

    // Track per-column remaining rowspan so that a `cell[rowspan:N]` in
    // an earlier row continues to consume cells in subsequent rows. The
    // source has no placeholder cells in continuation rows, so we
    // synthesize them as vMerge=continue cells matching the original
    // grid_span.
    let mut pending: Vec<RowspanCarry> = Vec::new();

    let mut table_rows: Vec<TableRow> = Vec::with_capacity(rows.len());
    let mut data_row_idx: usize = 0;
    for row in rows {
        if row.name != "row" {
            continue;
        }
        let is_header = row.prop_str("kind") == Some("header");
        let table_row = build_row(row, is_header, theme, &mut pending, stripe, data_row_idx);
        if !is_header {
            data_row_idx += 1;
        }
        table_rows.push(table_row);
    }

    let table = apply_table_borders(Table::new(table_rows), border_mode)
        .layout(docx_rs::TableLayoutType::Autofit);
    docx.add_table(table)
}

#[derive(Clone, Copy)]
enum BorderMode {
    None,
    Outer,
    All,
}

#[derive(Clone, Copy)]
struct RowspanCarry {
    /// Remaining rows (after this row's emission) that still need a
    /// continuation cell at this grid slot.
    remaining: u32,
    /// Width of the merged region in grid columns; the continuation
    /// cell must repeat the same grid_span.
    grid_span: usize,
}

fn build_row(
    row: &Block,
    is_header: bool,
    theme: &Theme,
    pending: &mut Vec<RowspanCarry>,
    stripe: bool,
    data_row_idx: usize,
) -> TableRow {
    let cell_blocks: &[Block] = match &row.body {
        Body::Children(c) => c.as_slice(),
        _ => &[],
    };

    let mut cells: Vec<TableCell> = Vec::new();
    let mut source_iter = cell_blocks.iter().filter(|c| c.name == "cell");
    let mut grid_col: usize = 0;

    // Walk grid columns left-to-right. For each column slot, either emit
    // a continuation cell (if a prior row's cell is still spanning) or
    // pull the next source cell.
    loop {
        // Continuation cells from prior rows.
        if grid_col < pending.len() && pending[grid_col].remaining > 0 {
            let carry = pending[grid_col];
            let cont = TableCell::new()
                .vertical_merge(VMergeType::Continue)
                .grid_span(carry.grid_span)
                .add_paragraph(Paragraph::new());
            cells.push(cont);
            // The continuation consumes `grid_span` grid columns; mark
            // them all as advanced and decrement remaining.
            for i in 0..carry.grid_span {
                if let Some(slot) = pending.get_mut(grid_col + i) {
                    slot.remaining = slot.remaining.saturating_sub(1);
                }
            }
            grid_col += carry.grid_span;
            continue;
        }

        let Some(source_cell) = source_iter.next() else {
            break;
        };
        let colspan: usize = source_cell
            .prop_str("colspan")
            .and_then(|s| s.parse().ok())
            .filter(|&n: &usize| n > 0)
            .unwrap_or(1);
        let rowspan: u32 = source_cell
            .prop_str("rowspan")
            .and_then(|s| s.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(1);

        let cell_force_bg = if is_header {
            None
        } else if stripe && data_row_idx % 2 == 1 {
            Some(STRIPE_FILL.to_string())
        } else {
            None
        };
        let mut cell = build_cell(source_cell, is_header, theme, cell_force_bg);
        if colspan > 1 {
            cell = cell.grid_span(colspan);
        }
        if rowspan > 1 {
            cell = cell.vertical_merge(VMergeType::Restart);
            // Reserve `colspan` slots in pending starting at grid_col.
            while pending.len() < grid_col + colspan {
                pending.push(RowspanCarry {
                    remaining: 0,
                    grid_span: 1,
                });
            }
            // Mark the leftmost slot as the carrier; mark inner slots as
            // already-merged (grid_span 1, remaining matches) so the
            // continuation logic doesn't double-emit. We model this by
            // putting the full carry at the leftmost slot and zero
            // remaining (but a placeholder span) on inner slots.
            pending[grid_col] = RowspanCarry {
                remaining: rowspan - 1,
                grid_span: colspan,
            };
            for i in 1..colspan {
                pending[grid_col + i] = RowspanCarry {
                    remaining: 0,
                    grid_span: 1,
                };
            }
        } else {
            // Ensure pending has slots for indexing even when no rowspan
            // is active so the continuation lookup is in-bounds.
            while pending.len() < grid_col + colspan {
                pending.push(RowspanCarry {
                    remaining: 0,
                    grid_span: 1,
                });
            }
        }
        cells.push(cell);
        grid_col += colspan;
    }

    // Trim trailing exhausted entries to keep pending compact.
    while matches!(
        pending.last(),
        Some(RowspanCarry { remaining: 0, .. })
    ) {
        pending.pop();
    }

    TableRow::new(cells)
}

fn build_cell(b: &Block, is_header: bool, theme: &Theme, force_bg: Option<String>) -> TableCell {
    let mut base = RunStyle::default();
    if is_header {
        base.bold = true;
    }

    let mut para = Paragraph::new();
    if let Some(align) = b.prop_str("align") {
        if let Some(a) = parse_alignment(align) {
            para = para.align(a);
        }
    }
    let runs = match &b.body {
        Body::Text(_) => collect_runs(b, base),
        Body::Children(children) => {
            // Cell may carry block children (non-standard but representable).
            // We flatten their text into the single paragraph; full block
            // rendering inside cells is a follow-up.
            let mut accumulated = Vec::new();
            for child in children {
                accumulated.extend(collect_runs(child, base));
            }
            accumulated
        }
        Body::None => Vec::new(),
    };
    for run in runs {
        para = para.add_run(run);
    }

    let mut cell = TableCell::new().add_paragraph(para);
    if let Some(valign) = b.prop_str("valign") {
        if let Some(v) = parse_valign(valign) {
            cell = cell.vertical_align(v);
        }
    }
    let bg_hex = force_bg.or_else(|| {
        b.prop_str("bg")
            .and_then(|name| theme.resolve_color(name))
            .map(|c| color_to_hex_no_hash(c.to_hex()))
    });
    if let Some(hex) = bg_hex {
        cell = cell.shading(
            Shading::new()
                .shd_type(docx_rs::ShdType::Clear)
                .color("auto")
                .fill(hex),
        );
    }
    cell
}

fn parse_alignment(s: &str) -> Option<AlignmentType> {
    match s {
        "left" => Some(AlignmentType::Left),
        "center" => Some(AlignmentType::Center),
        "right" => Some(AlignmentType::Right),
        "justify" => Some(AlignmentType::Justified),
        _ => None,
    }
}

fn parse_valign(s: &str) -> Option<VAlignType> {
    match s {
        "top" => Some(VAlignType::Top),
        "middle" => Some(VAlignType::Center),
        "bottom" => Some(VAlignType::Bottom),
        _ => None,
    }
}

/// Strip a leading `#` from a hex string for OOXML, which expects
/// `RRGGBB` (no `#`). Theme colors render as `#rrggbb`.
fn color_to_hex_no_hash(hex: String) -> String {
    hex.strip_prefix('#').unwrap_or(&hex).to_ascii_uppercase()
}

const STRIPE_FILL: &str = "F2F2F2";

fn apply_table_borders(table: Table, mode: BorderMode) -> Table {
    use docx_rs::TableBorderPosition as P;
    match mode {
        BorderMode::None => table.set_borders(TableBorders::with_empty()),
        BorderMode::All => {
            // Default TableBorders::new() already adds all six positions
            // (top/bottom/left/right/insideH/insideV) as Single 4-pt.
            table.set_borders(TableBorders::new())
        }
        BorderMode::Outer => {
            // Outer four sides single; inside lines cleared.
            let outer = |p: P| {
                TableBorder::new(p)
                    .border_type(BorderType::Single)
                    .size(4)
                    .color("auto")
            };
            let mut borders = TableBorders::with_empty()
                .set(outer(P::Top))
                .set(outer(P::Bottom))
                .set(outer(P::Left))
                .set(outer(P::Right));
            borders = borders
                .clear(P::InsideH)
                .clear(P::InsideV);
            table.set_borders(borders)
        }
    }
}

// --- list numbering definition ------------------------------------------

fn abstract_numbering(id: usize, bullet: bool) -> AbstractNumbering {
    // A 9-level list: each level either bulleted or decimal.
    let mut a = AbstractNumbering::new(id);
    for lvl in 0..9 {
        let text = if bullet {
            LevelText::new("•")
        } else {
            LevelText::new(format!("%{}.", lvl + 1))
        };
        let format = if bullet {
            NumberFormat::new("bullet")
        } else {
            NumberFormat::new("decimal")
        };
        let level = Level::new(lvl, Start::new(1), format, text, LevelJc::new("left")).indent(
            Some(720 * (lvl as i32 + 1)),
            Some(SpecialIndentType::Hanging(360)),
            None,
            None,
        );
        a = a.add_level(level);
    }
    a
}
