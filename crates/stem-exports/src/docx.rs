//! Stem AST → .docx via the `docx-rs` crate.
//!
//! MVP scope: headings (h1..h6 mapped to built-in Word styles
//! Heading1..6), paragraphs, ordered/unordered lists, blockquote,
//! fenced code blocks (Courier New), inline runs with italic/bold/
//! strikethrough.
//!
//! Out of scope: tables, images, links, footnotes, headers/footers,
//! page numbering, tracked changes, comments, theme/style customization.

use std::path::{Path, PathBuf};

use docx_rs::{
    AbstractNumbering, AlignmentType, BorderType, Docx, FieldCharType, Footer, Footnote, Header,
    Hyperlink, HyperlinkType, IndentLevel, InstrNUMPAGES, InstrPAGE, InstrText, Level, LevelJc,
    LevelText, NumberFormat, Numbering, NumberingId, PageMargin, PageOrientationType, Paragraph,
    Pic, Run, RunFonts, Shading, SpecialIndentType, Start, Style, StyleType, Table, TableBorder,
    TableBorders, TableCell, TableOfContents, TableRow, VAlignType, VMergeType,
};
use stem_core::ast::{Block, Body, Document, TextPiece};
use stem_core::theme::Theme;
use stem_core::Exporter;
use thiserror::Error;

#[derive(Default)]
pub struct DocxExporter {
    /// Directory used to resolve relative `image[src:..]` paths. When
    /// unset, relative paths resolve against the process's current
    /// working directory.
    image_base: Option<PathBuf>,
}

impl DocxExporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve relative image paths against `base`. Absolute paths are
    /// still used verbatim.
    pub fn with_image_base(mut self, base: impl AsRef<Path>) -> Self {
        self.image_base = Some(base.as_ref().to_path_buf());
        self
    }
}

#[derive(Debug, Error)]
pub enum DocxError {
    #[error("docx pack: {0}")]
    Pack(String),
    #[error("docx image: failed to read {path}: {source}")]
    ImageRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
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

        // Register the paragraph + character styles we reference from
        // the document body. docx-rs's default styles.xml only
        // defines `Normal` and `ToC1..6`, so Word can't apply
        // `Heading1`/`Caption`/`Hyperlink` etc. without these
        // explicit definitions — and the TOC field's heading scan
        // relies on the `outlineLvl` set here.
        out = register_styles(out);

        let ctx = EmitCtx {
            theme,
            image_base: self.image_base.as_deref(),
        };

        // Apply doc-level page setup from metadata. The schema gives
        // these names a stable home in the `[ ... ]` header at the
        // top of the file.
        out = apply_page_setup(out, &cooked.metadata)?;

        // Walk top-level blocks: `header` / `footer` go into the
        // section properties; everything else into the body.
        for block in &cooked.blocks {
            match block.name.as_str() {
                "header" => {
                    let header = build_header_footer_header(block, &ctx)?;
                    out = match block.prop_str("scope") {
                        Some("first") => out.first_header(header),
                        Some("even") => out.even_header(header),
                        _ => out.header(header),
                    };
                }
                "footer" => {
                    let footer = build_header_footer_footer(block, &ctx)?;
                    out = match block.prop_str("scope") {
                        Some("first") => out.first_footer(footer),
                        Some("even") => out.even_footer(footer),
                        _ => out.footer(footer),
                    };
                }
                _ => {
                    out = emit_block(out, block, &ctx, 0)?;
                }
            }
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

/// Per-export context that flows through every `emit_*` call. Holds the
/// theme (for color resolution) and the optional image base directory
/// used to resolve relative `image[src:..]` paths.
struct EmitCtx<'a> {
    theme: &'a Theme,
    image_base: Option<&'a Path>,
}

fn emit_block(docx: Docx, b: &Block, ctx: &EmitCtx, _depth: usize) -> Result<Docx, DocxError> {
    Ok(match b.name.as_str() {
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
        "table" => emit_table(docx, b, ctx.theme),
        "image" => emit_image(docx, b, ctx)?,
        "section" => emit_section(docx, b, ctx)?,
        _ => docx.add_paragraph(text_para(b, None)),
    })
}

/// Parse the optional `levels` property on `section[id:toc]`. Accepts
/// `"start-end"` (e.g. `1-3`) or a single value `"N"` meaning `1-N`.
/// Falls back to the full range supported by the schema. Out-of-range
/// or malformed values silently fall back too — the TOC is best-effort
/// metadata, not load-bearing structure.
fn parse_toc_levels(spec: Option<&str>) -> (usize, usize) {
    let max = stem_types::MAX_HEADING_LEVEL;
    let Some(s) = spec else { return (1, max) };
    let s = s.trim();
    if let Some((lo, hi)) = s.split_once('-') {
        let lo = lo.trim().parse::<usize>().ok().unwrap_or(1);
        let hi = hi.trim().parse::<usize>().ok().unwrap_or(max);
        let lo = lo.clamp(1, max);
        let hi = hi.clamp(lo, max);
        return (lo, hi);
    }
    if let Ok(n) = s.parse::<usize>() {
        return (1, n.clamp(1, max));
    }
    (1, max)
}

/// Render a `section[id:..]` block. The reserved id `toc` (see
/// `docs/schema.md` §2 `section.reserved[id:toc]`) emits an
/// auto-generated table of contents field that Word will populate when
/// the document is opened. Any other section is treated as a
/// transparent container — its child blocks are emitted in order,
/// preceded by a bookmark for cross-references.
fn emit_section(docx: Docx, b: &Block, ctx: &EmitCtx) -> Result<Docx, DocxError> {
    let id = b.prop_str("id");
    if id == Some("toc") {
        // TOC field matching what Word's "Insert > Table of Contents >
        // Automatic" produces. Switches:
        //   \o "start-end"  include heading range (default full
        //                   1..MAX_HEADING_LEVEL; overridable via
        //                   `levels` property).
        //   \h              every entry is a hyperlink (clickable).
        //   \z              hide tab leader and page numbers in Web
        //                   layout view.
        //   \u              use applied paragraph outline level — lets
        //                   the field rely on the `outlineLvl` set in
        //                   register_styles rather than per-paragraph
        //                   numPr.
        // `dirty` triggers Word to refresh the field on open. We drop
        // the SDT wrapper to match the BoringCrypto reference and
        // avoid Word's content-controls dialog.
        // docx-rs's TableOfContents builder doesn't surface \u or \z,
        // so we hand-build the instruction text via `with_instr_text`.
        let (start, end) = parse_toc_levels(b.prop_str("levels"));
        let instr = format!(" TOC \\o \"{}-{}\" \\h \\z \\u ", start, end);
        let toc = TableOfContents::with_instr_text(&instr)
            .dirty()
            .without_sdt()
            .add_before_paragraph(
                Paragraph::new()
                    .style("Heading1")
                    .add_run(Run::new().add_text("Table of Contents")),
            );
        return Ok(docx.add_table_of_contents(toc));
    }
    let mut docx = docx;
    if let Body::Children(children) = &b.body {
        for child in children {
            docx = emit_block(docx, child, ctx, 0)?;
        }
    }
    Ok(docx)
}

fn heading_para(b: &Block, level: u8) -> Paragraph {
    let style = format!("Heading{}", level);
    let p = Paragraph::new().style(&style);
    apply_pieces(p, collect_pieces(b, RunStyle::default()))
}

fn text_para(b: &Block, style: Option<&str>) -> Paragraph {
    let mut p = Paragraph::new();
    if let Some(s) = style {
        p = p.style(s);
    }
    apply_pieces(p, collect_pieces(b, RunStyle::default()))
}

fn blockquote_para(b: &Block) -> Paragraph {
    let style = RunStyle {
        italic: true,
        ..RunStyle::default()
    };
    let p = Paragraph::new().indent(Some(720), None, None, None);
    apply_pieces(p, collect_pieces(b, style))
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
            let p = Paragraph::new().numbering(NumberingId::new(num_id), IndentLevel::new(0));
            let p = apply_pieces(p, collect_pieces(item, RunStyle::default()));
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

/// One piece of paragraph content. Most text becomes `Run`, but `@link`
/// becomes `Hyperlink` (wraps its own runs) and `@footnote` becomes a
/// run carrying a `FootnoteReference`. Splitting the cases lets callers
/// drive `Paragraph::add_run` vs `Paragraph::add_hyperlink` correctly.
enum InlinePiece {
    Run(Run),
    Hyperlink(Hyperlink),
}

/// Walk a block's text body and produce inline pieces with styling
/// inherited from `base` and overridden by inline elements.
fn collect_pieces(b: &Block, base: RunStyle) -> Vec<InlinePiece> {
    let mut out: Vec<InlinePiece> = Vec::new();
    if let Body::Text(pieces) = &b.body {
        for piece in pieces {
            match piece {
                TextPiece::Literal { text, .. } => {
                    out.push(InlinePiece::Run(styled_run(text, base)));
                }
                TextPiece::Inline(inline) => match inline.name.as_str() {
                    "link" => {
                        if let Some(link) = build_hyperlink(inline, base) {
                            out.push(InlinePiece::Hyperlink(link));
                        }
                    }
                    "footnote" => {
                        if let Some(run) = build_footnote_marker(inline, base) {
                            out.push(InlinePiece::Run(run));
                        }
                    }
                    "page-number" => {
                        out.push(InlinePiece::Run(page_field_run(PageFieldKind::Page)));
                    }
                    "total-pages" => {
                        out.push(InlinePiece::Run(page_field_run(PageFieldKind::TotalPages)));
                    }
                    _ => {
                        let style = inline_style(inline, base);
                        let text = inline.plain_text().unwrap_or_default();
                        if !text.is_empty() {
                            out.push(InlinePiece::Run(styled_run(&text, style)));
                        }
                    }
                },
            }
        }
    }
    out
}

/// Drain `pieces` into the paragraph, dispatching by piece kind.
fn apply_pieces(mut p: Paragraph, pieces: Vec<InlinePiece>) -> Paragraph {
    for piece in pieces {
        match piece {
            InlinePiece::Run(r) => p = p.add_run(r),
            InlinePiece::Hyperlink(h) => p = p.add_hyperlink(h),
        }
    }
    p
}

/// Build a hyperlink from `@link[to:"…"](text)`. Internal references
/// (`ref://anchor` or `#anchor`) become anchor hyperlinks; everything
/// else is treated as external.
fn build_hyperlink(b: &Block, base: RunStyle) -> Option<Hyperlink> {
    let to_raw = b.prop_str("to")?;
    let (target, kind) = parse_link_target(to_raw);
    // Visible text: prefer plain text body; fall back to the target.
    let visible = b.plain_text().unwrap_or_else(|| target.clone());
    // Word renders Hyperlink-styled runs as blue+underlined when the
    // "Hyperlink" character style is registered; we don't define it
    // explicitly so the run carries the style name only.
    let mut style = base;
    style.italic = base.italic; // leave style as-is; visual handled by Word
    let run = styled_run(&visible, style).style("Hyperlink");
    Some(Hyperlink::new(target, kind).add_run(run))
}

fn parse_link_target(s: &str) -> (String, HyperlinkType) {
    if let Some(anchor) = s.strip_prefix("ref://") {
        return (anchor.to_string(), HyperlinkType::Anchor);
    }
    if let Some(anchor) = s.strip_prefix('#') {
        return (anchor.to_string(), HyperlinkType::Anchor);
    }
    (s.to_string(), HyperlinkType::External)
}

/// Build a run carrying a footnote reference. The footnote's content
/// is the inline body text.
fn build_footnote_marker(b: &Block, base: RunStyle) -> Option<Run> {
    let text = b.plain_text().unwrap_or_default();
    if text.is_empty() {
        return None;
    }
    let mut footnote = Footnote::new();
    footnote = footnote.add_content(Paragraph::new().add_run(Run::new().add_text(text)));
    // The reference itself is a small superscript marker; docx-rs sets
    // the run's style to "FootnoteReference" via add_footnote_reference.
    let _ = base; // footnote markers don't inherit bold/italic from surrounding text
    Some(Run::new().add_footnote_reference(footnote))
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
        docx = docx.add_paragraph(caption_paragraph(CaptionKind::Table, caption));
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
    let pieces = match &b.body {
        Body::Text(_) => collect_pieces(b, base),
        Body::Children(children) => {
            // Cell may carry block children (non-standard but representable).
            // We flatten their text into the single paragraph; full block
            // rendering inside cells is a follow-up.
            let mut accumulated = Vec::new();
            for child in children {
                accumulated.extend(collect_pieces(child, base));
            }
            accumulated
        }
        Body::None => Vec::new(),
    };
    para = apply_pieces(para, pieces);

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

// --- page setup / headers / footers (stage 4) ---------------------------

/// Read `page-size`, `orientation`, and `margin` keys from the document
/// metadata and apply them to the Word section properties. Unknown
/// values fall back to A4 portrait + 1 inch margins (matching
/// docx-rs's own defaults).
fn apply_page_setup(mut docx: Docx, metadata: &stem_core::ast::Metadata) -> Result<Docx, DocxError> {
    // Page size — handled as a logical name plus an optional explicit
    // override via `page-w` / `page-h` lengths.
    let (mut w_twips, mut h_twips) = match metadata.get_str("page-size") {
        Some("letter") => (12_240u32, 15_840u32),       // 8.5" x 11"
        Some("legal") => (12_240, 20_160),               // 8.5" x 14"
        Some("tabloid") => (15_840, 24_480),             // 11" x 17"
        Some("a4") => (11_906, 16_838),                  // 210 x 297mm
        Some("a3") => (16_838, 23_811),
        Some("a5") => (8_391, 11_906),
        Some("b5") => (9_977, 14_175),
        _ => (11_906, 16_838),                            // default A4
    };
    let orientation = match metadata.get_str("orientation") {
        Some("landscape") => Some(PageOrientationType::Landscape),
        Some("portrait") => Some(PageOrientationType::Portrait),
        _ => None,
    };
    if matches!(orientation, Some(PageOrientationType::Landscape)) {
        std::mem::swap(&mut w_twips, &mut h_twips);
    }
    docx = docx.page_size(w_twips, h_twips);
    if let Some(o) = orientation {
        docx = docx.page_orient(o);
    }

    // Margin: single length applies to all four sides; four
    // whitespace-separated lengths apply in CSS shorthand order
    // (top right bottom left).
    if let Some(margin_str) = metadata.get_str("margin") {
        let twips = parse_margins_twips(margin_str);
        if let Some((t, r, b, l)) = twips {
            let m = PageMargin::new()
                .top(t)
                .right(r)
                .bottom(b)
                .left(l)
                .header(708)
                .footer(708);
            docx = docx.page_margin(m);
        }
    }
    Ok(docx)
}

fn parse_margins_twips(s: &str) -> Option<(i32, i32, i32, i32)> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    let to_twips = |p: &str| parse_length_to_twips(p);
    match parts.len() {
        1 => {
            let v = to_twips(parts[0])?;
            Some((v, v, v, v))
        }
        2 => {
            let v = to_twips(parts[0])?;
            let h = to_twips(parts[1])?;
            Some((v, h, v, h))
        }
        4 => {
            let t = to_twips(parts[0])?;
            let r = to_twips(parts[1])?;
            let b = to_twips(parts[2])?;
            let l = to_twips(parts[3])?;
            Some((t, r, b, l))
        }
        _ => None,
    }
}

/// Parse a length to twips (1/1440 inch). Same suffix grammar as the
/// image length parser; pixels default to 96 DPI.
fn parse_length_to_twips(s: &str) -> Option<i32> {
    let s = s.trim();
    if s.ends_with('%') {
        return None;
    }
    let (num, unit) = split_length(s)?;
    let value: f64 = num.parse().ok()?;
    let twips_per_unit = match unit {
        "" | "in" => 1440.0,
        "pt" => 20.0,
        "cm" => 567.0,
        "mm" => 56.7,
        "px" => 15.0,
        _ => return None,
    };
    Some((value * twips_per_unit).round() as i32)
}

fn build_header_footer_header(b: &Block, ctx: &EmitCtx) -> Result<Header, DocxError> {
    let mut h = Header::new();
    for para in build_paragraphs_for_header_footer(b, ctx)? {
        h = h.add_paragraph(para);
    }
    Ok(h)
}

fn build_header_footer_footer(b: &Block, ctx: &EmitCtx) -> Result<Footer, DocxError> {
    let mut f = Footer::new();
    for para in build_paragraphs_for_header_footer(b, ctx)? {
        f = f.add_paragraph(para);
    }
    Ok(f)
}

/// Walk a `header`/`footer` body and produce one Paragraph per child
/// block. Only paragraph-like blocks are supported here (p, h*, ul/ol
/// flattened to text); a full block emitter would require a deeper
/// refactor since Header/Footer don't accept the same wide API as the
/// main body. This covers the common "one centered title line + a
/// page counter" use case.
fn build_paragraphs_for_header_footer(b: &Block, _ctx: &EmitCtx) -> Result<Vec<Paragraph>, DocxError> {
    let mut paras = Vec::new();
    if let Body::Children(children) = &b.body {
        for child in children {
            // Treat any text-bearing block as a paragraph with the
            // child's runs (and any embedded page-number / total-pages
            // fields). This keeps headers/footers simple but works for
            // the academic-paper target.
            let p = Paragraph::new();
            let p = apply_pieces(p, collect_pieces(child, RunStyle::default()));
            paras.push(p);
        }
    } else if matches!(&b.body, Body::Text(_)) {
        // Allow `header(Some title text)` shorthand.
        let p = Paragraph::new();
        let p = apply_pieces(p, collect_pieces(b, RunStyle::default()));
        paras.push(p);
    }
    Ok(paras)
}

/// Build a `<w:r>` that wraps a PAGE or NUMPAGES field. Uses the
/// complex-field form (begin/instrText/end) so Word will recalculate
/// the value on open.
fn page_field_run(kind: PageFieldKind) -> Run {
    let instr = match kind {
        PageFieldKind::Page => InstrText::PAGE(InstrPAGE::new()),
        PageFieldKind::TotalPages => InstrText::NUMPAGES(InstrNUMPAGES::new()),
    };
    Run::new()
        .add_field_char(FieldCharType::Begin, false)
        .add_instr_text(instr)
        .add_field_char(FieldCharType::End, false)
}

enum PageFieldKind {
    Page,
    TotalPages,
}

// --- images -------------------------------------------------------------

/// Render an `image[src:.., alt:.., w:.., h:.., caption:..]` block as
/// an inline picture inside its own paragraph, optionally followed by a
/// Caption-styled paragraph.
///
/// `src` is read from disk. Relative paths resolve against
/// `ctx.image_base` if set, otherwise the process cwd. The image bytes
/// are passed to docx-rs's `Pic::new` which auto-detects the format and
/// computes pixel dimensions; we override the EMU size if explicit
/// `w`/`h` lengths are given.
///
/// `alt` is currently dropped — docx-rs's Pic API doesn't expose
/// `wp:docPr/@title|@descr` so the accessibility attribute can't be set
/// without bypassing the builder. Tracked as a follow-up.
fn emit_image(docx: Docx, b: &Block, ctx: &EmitCtx) -> Result<Docx, DocxError> {
    let Some(src) = b.prop_str("src") else {
        return Ok(docx.add_paragraph(
            Paragraph::new().add_run(Run::new().add_text("[image: missing src]")),
        ));
    };

    let resolved = resolve_image_path(src, ctx.image_base);
    let bytes = std::fs::read(&resolved).map_err(|e| DocxError::ImageRead {
        path: resolved.clone(),
        source: e,
    })?;

    // Pic::new (with docx-rs's default `image` feature) decodes,
    // normalises to PNG, and reports native pixel dimensions.
    let mut pic = Pic::new(&bytes);

    // Override the EMU size if explicit width/height are given. Both
    // axes must be present to scale freely; if only one is set we
    // preserve the original aspect ratio.
    let want_w = b.prop_str("w").and_then(parse_length_to_emu);
    let want_h = b.prop_str("h").and_then(parse_length_to_emu);
    let (orig_w_emu, orig_h_emu) = pic.size;
    match (want_w, want_h) {
        (Some(w), Some(h)) => pic = pic.size(w, h),
        (Some(w), None) => {
            let h = scale_axis(orig_h_emu, orig_w_emu, w);
            pic = pic.size(w, h);
        }
        (None, Some(h)) => {
            let w = scale_axis(orig_w_emu, orig_h_emu, h);
            pic = pic.size(w, h);
        }
        (None, None) => {}
    }

    let mut docx = docx.add_paragraph(Paragraph::new().add_run(Run::new().add_image(pic)));
    if let Some(caption) = b.prop_str("caption") {
        docx = docx.add_paragraph(caption_paragraph(CaptionKind::Figure, caption));
    }
    Ok(docx)
}

// --- caption helpers ----------------------------------------------------

#[derive(Clone, Copy)]
enum CaptionKind {
    Figure,
    Table,
}

impl CaptionKind {
    fn label(self) -> &'static str {
        match self {
            CaptionKind::Figure => "Figure ",
            CaptionKind::Table => "Table ",
        }
    }
    fn seq_name(self) -> &'static str {
        match self {
            CaptionKind::Figure => "Figure",
            CaptionKind::Table => "Table",
        }
    }
}

/// Build the canonical "Figure N. <text>" / "Table N. <text>" caption
/// paragraph, with the number rendered as a live `SEQ` field that
/// Word increments per occurrence in document order.
fn caption_paragraph(kind: CaptionKind, text: &str) -> Paragraph {
    let instr = InstrText::Unsupported(format!(" SEQ {} \\* ARABIC ", kind.seq_name()));
    let seq_run = Run::new()
        .add_field_char(FieldCharType::Begin, false)
        .add_instr_text(instr)
        .add_field_char(FieldCharType::End, false);
    Paragraph::new()
        .style("Caption")
        .add_run(Run::new().add_text(kind.label()))
        .add_run(seq_run)
        .add_run(Run::new().add_text(format!(". {}", text)))
}

fn resolve_image_path(src: &str, base: Option<&Path>) -> PathBuf {
    let p = Path::new(src);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    match base {
        Some(b) => b.join(p),
        None => p.to_path_buf(),
    }
}

/// Scale `axis_a` proportionally given the new value of `axis_b` and
/// the original values. Used when only one of `w`/`h` is specified.
fn scale_axis(orig_a: u32, orig_b: u32, new_b: u32) -> u32 {
    if orig_b == 0 {
        return orig_a;
    }
    ((orig_a as u64 * new_b as u64) / orig_b as u64) as u32
}

/// Parse a length string into EMU. Accepts a bare number (treated as
/// pixels at 96 DPI) or a number with a `px`, `pt`, `in`, `cm`, or
/// `mm` suffix. Percentages are not supported and yield None — the
/// caller falls back to the image's native size.
fn parse_length_to_emu(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.ends_with('%') {
        return None;
    }
    let (num, unit) = split_length(s)?;
    let value: f64 = num.parse().ok()?;
    let emu_per_unit = match unit {
        "" | "px" => 9525.0,        // 96 DPI
        "pt" => 12700.0,
        "in" => 914400.0,
        "cm" => 360000.0,
        "mm" => 36000.0,
        _ => return None,
    };
    let emu = (value * emu_per_unit).round();
    if emu < 0.0 {
        return None;
    }
    Some(emu as u32)
}

fn split_length(s: &str) -> Option<(&str, &str)> {
    // Find the boundary between numeric chars and trailing alpha.
    let idx = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());
    let (num, unit) = s.split_at(idx);
    if num.is_empty() {
        return None;
    }
    Some((num, unit))
}

// --- styles -------------------------------------------------------------

/// Decreasing heading sizes, in OOXML half-points. Index 0 = `Heading1`.
/// 36/32/28/26/24/22 hp = 18/16/14/13/12/11 pt; clamps at 22 hp for
/// anything past the array.
fn heading_size_half_points(level: usize) -> usize {
    const SIZES: &[usize] = &[36, 32, 28, 26, 24, 22];
    *SIZES
        .get(level.saturating_sub(1))
        .unwrap_or_else(|| SIZES.last().expect("SIZES is non-empty"))
}


/// Define the paragraph + character styles we reference by ID from the
/// body. Without these, Word may render the document but features
/// that look up styles by ID — most importantly the TOC field, which
/// scans for `outlineLvl` on heading styles — silently produce no
/// content.
///
/// Mirrors the subset of Word's built-in style names so a third-party
/// docx reader that ships its own defaults still picks up reasonable
/// formatting.
fn register_styles(mut docx: Docx) -> Docx {
    // Heading1..N with outline levels 0..N-1. The level cap comes from
    // `stem_types::MAX_HEADING_LEVEL` so this stays in sync with the
    // h1..hN element definitions and the default TOC heading range.
    // Sizes step from 36pt down to 22pt in half-point units.
    for level in 1..=stem_types::MAX_HEADING_LEVEL {
        let size_hp = heading_size_half_points(level);
        let style = Style::new(format!("Heading{}", level), StyleType::Paragraph)
            .name(format!("heading {}", level))
            .based_on("Normal")
            .next("Normal")
            .q_format(true)
            .ui_priority(9)
            .outline_lvl(level - 1)
            .bold()
            .size(size_hp);
        docx = docx.add_style(style);
    }

    // Title: typically used on the cover page; larger than Heading1.
    let title = Style::new("Title", StyleType::Paragraph)
        .name("Title")
        .based_on("Normal")
        .next("Normal")
        .q_format(true)
        .ui_priority(10)
        .bold()
        .size(56);
    docx = docx.add_style(title);

    // Caption — italic, slightly smaller. Word's built-in default
    // looks similar.
    let caption = Style::new("Caption", StyleType::Paragraph)
        .name("caption")
        .based_on("Normal")
        .next("Normal")
        .q_format(true)
        .ui_priority(35)
        .italic()
        .size(18);
    docx = docx.add_style(caption);

    // Hyperlink: blue + underlined character style applied to runs
    // inside <w:hyperlink>. Without this, the hyperlink visually
    // looks like plain text.
    let hyperlink = Style::new("Hyperlink", StyleType::Character)
        .name("Hyperlink")
        .ui_priority(99)
        .color("0563C1")
        .underline("single");
    docx = docx.add_style(hyperlink);

    // FootnoteReference: superscript marker character style.
    let foot_ref = Style::new("FootnoteReference", StyleType::Character)
        .name("footnote reference")
        .ui_priority(99)
        .size(16);
    docx = docx.add_style(foot_ref);

    docx
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
