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
    AbstractNumbering, AlignmentType, BorderType, BreakType, Docx, FieldCharType, Footer, Footnote, Header,
    Hyperlink, HyperlinkType, IndentLevel, InstrNUMPAGES, InstrPAGE, InstrText, Level, LevelJc,
    LevelText, LineSpacing, LineSpacingType, NumberFormat, Numbering, NumberingId, PageMargin,
    PageOrientationType, Paragraph, Pic, Run, RunFonts, Shading, SpecialIndentType, Start, Style,
    StyleType, Table, TableBorder, TableBorders, TableCell, TableOfContents, TableRow, VAlignType,
    VMergeType,
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
            .add_numbering(Numbering::new(2, 2))
            // Heading multi-level numbering: 1., 1.1, 1.1.1, ... up
            // to MAX_HEADING_LEVEL. Referenced from heading paragraphs
            // that carry `numbered:true`.
            .add_abstract_numbering(heading_abstract_numbering(NUM_ID_HEADING))
            .add_numbering(Numbering::new(NUM_ID_HEADING, NUM_ID_HEADING));

        // Document defaults — applied to every paragraph unless
        // overridden. Matches Word's modern Normal defaults: Calibri
        // 11pt body, 1.08× line height, 8pt trailing space so
        // paragraphs visually separate.
        out = out
            .default_size(22)
            .default_fonts(
                // Set both ascii/hi_ansi and east_asia/cs explicitly so
                // localized Word builds (Korean Word, Japanese Word, etc.)
                // don't pick a system fallback like Malgun Gothic for
                // East-Asian-script paragraphs.
                RunFonts::new()
                    .ascii("Calibri")
                    .hi_ansi("Calibri")
                    .east_asia("Calibri")
                    .cs("Calibri"),
            )
            .default_line_spacing(
                LineSpacing::new()
                    .line_rule(LineSpacingType::Auto)
                    .line(259)
                    .after(160),
            );

        // Register the paragraph + character styles we reference from
        // the document body. docx-rs's default styles.xml only
        // defines `Normal` and `ToC1..6`, so Word can't apply
        // `Heading1`/`Caption`/`Hyperlink` etc. without these
        // explicit definitions — and the TOC field's heading scan
        // relies on the `outlineLvl` set here.
        out = register_styles(out);

        let headings = collect_headings(&cooked.blocks);
        let captions = collect_captions(&cooked.blocks);
        let n_headings = headings.len();
        let ctx = EmitCtx {
            theme,
            image_base: self.image_base.as_deref(),
            headings,
            heading_cursor: std::cell::Cell::new(0),
            // Reserve IDs 1..=n_headings for heading bookmarks; caption
            // bookmarks continue past that.
            bookmark_id: std::cell::Cell::new(n_headings + 1),
            table_caption_seq: std::cell::Cell::new(0),
            figure_caption_seq: std::cell::Cell::new(0),
            captions,
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
        // Repair docx-rs's OOXML child-ordering bugs in-place. Word
        // renders headings/TOC/styles correctly only when each parent
        // element's children follow the schema order; docx-rs does
        // not enforce this for `<w:pPr>` and `<w:style>`.
        repair_ooxml_ordering(buf)
    }
}

// --- emission -----------------------------------------------------------

/// Per-export context that flows through every `emit_*` call. Holds the
/// theme (for color resolution), the optional image base directory
/// used to resolve relative `image[src:..]` paths, and the pre-walked
/// list of heading anchors used to bookmark each heading and pre-
/// populate the TOC field.
struct EmitCtx<'a> {
    theme: &'a Theme,
    image_base: Option<&'a Path>,
    headings: Vec<HeadingInfo>,
    /// Mutable counter used during emission to pair each heading
    /// paragraph with its pre-assigned bookmark. Wrapped in a Cell so
    /// `&EmitCtx` callers can advance it.
    heading_cursor: std::cell::Cell<usize>,
    /// Bookmark id counter shared across heading + caption emission.
    bookmark_id: std::cell::Cell<usize>,
    /// Per-caption-kind sequence counters for `_Toc_table_N` /
    /// `_Toc_figure_N` bookmark names — these let Word "Insert Table
    /// of Tables/Figures" find each caption.
    table_caption_seq: std::cell::Cell<usize>,
    figure_caption_seq: std::cell::Cell<usize>,
    /// Pre-walked caption outline. Currently retained so a future
    /// pass can pre-populate the List-of-Tables / List-of-Figures
    /// fields with explicit items styled `TableofFigures` (the
    /// reference uses that style for both lists; docx-rs's
    /// `TableOfContentsItem` is locked to `ToC{level}`).
    #[allow(dead_code)]
    captions: Vec<CaptionInfo>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct CaptionInfo {
    kind: CaptionKindTag,
    text: String,
    bookmark: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptionKindTag {
    Table,
    Figure,
}

fn collect_captions(blocks: &[Block]) -> Vec<CaptionInfo> {
    let mut out: Vec<CaptionInfo> = Vec::new();
    fn walk(blocks: &[Block], out: &mut Vec<CaptionInfo>) {
        let mut t_seq = 0;
        let mut f_seq = 0;
        for b in blocks {
            match b.name.as_str() {
                "table" => {
                    if let Some(c) = b.prop_str("caption") {
                        t_seq += 1;
                        out.push(CaptionInfo {
                            kind: CaptionKindTag::Table,
                            text: c.to_string(),
                            bookmark: format!("_Toc_table_{}", t_seq),
                        });
                    }
                }
                "image" => {
                    if let Some(c) = b.prop_str("caption") {
                        f_seq += 1;
                        out.push(CaptionInfo {
                            kind: CaptionKindTag::Figure,
                            text: c.to_string(),
                            bookmark: format!("_Toc_figure_{}", f_seq),
                        });
                    }
                }
                _ => {
                    if let Body::Children(children) = &b.body {
                        walk(children, out);
                    }
                }
            }
        }
        // Restore counters by depth-position — emitting is in document
        // order so we don't strictly need scoping.
        let _ = (t_seq, f_seq);
    }
    walk(blocks, &mut out);
    out
}

/// One entry in the pre-walked heading outline. The bookmark name is
/// deterministic (`_Toc_h{level}_{seq}`) so successive renders of the
/// same source produce byte-stable output, and the TOC field's
/// PAGEREF entries can hyperlink to the same name the heading
/// paragraph emits.
#[derive(Clone)]
struct HeadingInfo {
    level: usize,
    text: String,
    bookmark: String,
}

/// Pre-walk the block tree and collect heading metadata in document
/// order. Walks transparently through `section` containers since they
/// wrap headings without changing outline semantics.
fn collect_headings(blocks: &[Block]) -> Vec<HeadingInfo> {
    let mut out: Vec<HeadingInfo> = Vec::new();
    fn walk(blocks: &[Block], out: &mut Vec<HeadingInfo>) {
        for b in blocks {
            match b.name.as_str() {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level: usize = b.name[1..].parse().unwrap_or(1);
                    let text = b.plain_text().unwrap_or_default();
                    let seq = out.len() + 1;
                    let bookmark = format!("_Toc_h{}_{}", level, seq);
                    out.push(HeadingInfo {
                        level,
                        text,
                        bookmark,
                    });
                }
                "section" => {
                    if let Body::Children(children) = &b.body {
                        walk(children, out);
                    }
                }
                _ => {}
            }
        }
    }
    walk(blocks, &mut out);
    out
}

fn emit_block(docx: Docx, b: &Block, ctx: &EmitCtx, _depth: usize) -> Result<Docx, DocxError> {
    Ok(match b.name.as_str() {
        "h1" => docx.add_paragraph(heading_para(b, 1, ctx)),
        "h2" => docx.add_paragraph(heading_para(b, 2, ctx)),
        "h3" => docx.add_paragraph(heading_para(b, 3, ctx)),
        "h4" => docx.add_paragraph(heading_para(b, 4, ctx)),
        "h5" => docx.add_paragraph(heading_para(b, 5, ctx)),
        "h6" => docx.add_paragraph(heading_para(b, 6, ctx)),
        "p" => docx.add_paragraph(text_para(b, None)),
        "blockquote" => docx.add_paragraph(blockquote_para(b)),
        "code" => emit_code_block(docx, b),
        "ol" => emit_list(docx, b, true),
        "ul" => emit_list(docx, b, false),
        "hr" => docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text("───"))),
        "pagebreak" => docx.add_paragraph(
            Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
        ),
        "table" => emit_table(docx, b, ctx),
        "image" => emit_image(docx, b, ctx)?,
        "section" => emit_section(docx, b, ctx)?,
        "title" => docx.add_paragraph(title_para(b)),
        "caption" => docx.add_paragraph(
            // Standalone Caption paragraph — body text becomes the
            // caption text, no SEQ field. Mirrors the reference's
            // empty caption placeholder between Table 5 and Table 6.
            Paragraph::new()
                .style("Caption")
                .add_run({
                    let text = b.plain_text().unwrap_or_default();
                    Run::new().add_text(text)
                }),
        ),
        _ => docx.add_paragraph(text_para(b, None)),
    })
}

/// Hand-build the List-of-Tables / List-of-Figures field. docx-rs's
/// TableOfContents API ties each item to a ToC{level} paragraph style;
/// the reference uses `TableofFigures` for both lists. We bypass the
/// builder and emit the field as plain Word paragraphs:
///
///   <p style="TOCHeading">List of Tables</p>
///   <p style="TableofFigures">
///     <r><fldChar begin/></r>
///     <r><instrText> TOC \h \z \c "Table" </instrText></r>
///     <r><fldChar separate/></r>
///     <hyperlink anchor="_Toc_table_1"><r>Table 1 – …</r></hyperlink>
///     ...
///     <r><fldChar end/></r>
///   </p>
fn emit_caption_toc(docx: Docx, ctx: &EmitCtx, kind: CaptionKindTag) -> Docx {
    let label_word = match kind {
        CaptionKindTag::Table => "Table",
        CaptionKindTag::Figure => "Figure",
    };
    let heading_label = match kind {
        CaptionKindTag::Table => "List of Tables",
        CaptionKindTag::Figure => "List of Figures",
    };
    let mut docx = docx.add_paragraph(
        Paragraph::new()
            .style("TOCHeading")
            .add_run(Run::new().add_text(heading_label)),
    );

    let items: Vec<&CaptionInfo> = ctx.captions.iter().filter(|c| c.kind == kind).collect();
    if items.is_empty() {
        return docx;
    }

    // First entry paragraph also carries the field begin/instr/separate.
    let instr = format!(" TOC \\h \\z \\c \"{}\" ", label_word);
    for (i, info) in items.iter().enumerate() {
        let mut p = Paragraph::new().style("TableofFigures");
        if i == 0 {
            p = p.add_run(
                Run::new()
                    .add_field_char(FieldCharType::Begin, true)
                    .add_instr_text(InstrText::Unsupported(instr.clone()))
                    .add_field_char(FieldCharType::Separate, false),
            );
        }
        // Build the visible "Table N. <text>" — the SEQ number is a
        // hard literal here (we'd need to track caption numbers
        // separately). Use the sequence index + 1.
        let display = format!("{} {}. {}", label_word, i + 1, info.text);
        // Wrap in a hyperlink to the caption's bookmark.
        let link = Hyperlink::new(info.bookmark.clone(), HyperlinkType::Anchor)
            .add_run(Run::new().add_text(display));
        p = p.add_hyperlink(link);
        if i == items.len() - 1 {
            p = p.add_run(Run::new().add_field_char(FieldCharType::End, false));
        }
        docx = docx.add_paragraph(p);
    }
    docx
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
        let (start, end) = parse_toc_levels(b.prop_str("levels"));
        // TOC field instruction. Switches mirror what Word's "Insert
        // > Table of Contents > Automatic" produces.
        let instr = format!(" TOC \\o \"{}-{}\" \\h \\z \\u ", start, end);
        let mut toc = TableOfContents::with_instr_text(&instr)
            .dirty()
            .without_sdt()
            .add_before_paragraph(
                Paragraph::new()
                    .style("TOCHeading")
                    .add_run(Run::new().add_text("Table of Contents")),
            );
        // Pre-populate TOC entries from the document outline so Word
        // shows a complete table on first open. The field is still
        // marked dirty so right-click "Update Field" recomputes the
        // page numbers (which we can't know up front).
        for info in &ctx.headings {
            if info.level < start || info.level > end {
                continue;
            }
            let item = docx_rs::TableOfContentsItem::new()
                .text(info.text.clone())
                .level(info.level)
                .toc_key(info.bookmark.clone());
            toc = toc.add_item(item);
        }
        // docx-rs always appends an extra TOC{N}-styled paragraph
        // carrying the field's `end` marker. The repair pass strips
        // its pStyle so the heading-style count stays correct.
        return Ok(docx.add_table_of_contents(toc));
    }
    // Reserved IDs for caption-based TOCs (List of Tables / Figures).
    if id == Some("list-of-tables") || id == Some("list-of-figures") {
        let kind = if id == Some("list-of-tables") {
            CaptionKindTag::Table
        } else {
            CaptionKindTag::Figure
        };
        return Ok(emit_caption_toc(docx, ctx, kind));
    }
    let mut docx = docx;
    if let Body::Children(children) = &b.body {
        for child in children {
            docx = emit_block(docx, child, ctx, 0)?;
        }
    }
    Ok(docx)
}

fn title_para(b: &Block) -> Paragraph {
    let mut p = Paragraph::new()
        .style("Title")
        // Tighter spacing than the body default (8pt after). The
        // BoringCrypto reference uses spacing-after=120 (6pt) and
        // a single-line height on Title paragraphs.
        .line_spacing(
            LineSpacing::new()
                .line_rule(LineSpacingType::Auto)
                .line(240)
                .after(120),
        );
    if let Some(align) = b.prop_str("align") {
        if let Some(a) = parse_alignment(align) {
            p = p.align(a);
        }
    } else {
        p = p.align(AlignmentType::Center);
    }
    apply_pieces(p, collect_pieces(b, RunStyle::default()))
}

fn heading_para(b: &Block, level: u8, ctx: &EmitCtx) -> Paragraph {
    let style = format!("Heading{}", level);
    let mut p = Paragraph::new()
        .style(&style)
        .keep_next(true)
        .keep_lines(true)
        .line_spacing(heading_paragraph_spacing(level as usize));
    if b.prop_str("numbered") == Some("true") {
        p = p.numbering(
            NumberingId::new(NUM_ID_HEADING),
            IndentLevel::new((level as usize).saturating_sub(1)),
        );
    }
    // Bookmark this heading so the TOC field's PAGEREF entries can
    // anchor here. The cursor was populated by `collect_headings` in
    // document order, so advancing it once per heading paragraph keeps
    // the names in sync with the pre-populated TOC items.
    let cursor = ctx.heading_cursor.get();
    if let Some(info) = ctx.headings.get(cursor) {
        // Bookmark IDs are 1-based and must be unique per document;
        // we use the cursor index + 1.
        let id = cursor + 1;
        p = p
            .add_bookmark_start(id, info.bookmark.clone())
            .add_bookmark_end(id);
    }
    ctx.heading_cursor.set(cursor + 1);
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
fn emit_table(docx: Docx, b: &Block, ctx: &EmitCtx) -> Docx {
    let theme = ctx.theme;
    let border_mode = match b.prop_str("border").unwrap_or("none") {
        "all" => BorderMode::All,
        "outer" => BorderMode::Outer,
        _ => BorderMode::None,
    };
    let stripe = b.prop_str("stripe").map(|v| v == "true").unwrap_or(false);

    let mut docx = docx;
    let caption_text = b.prop_str("caption").map(str::to_string);

    let rows = match &b.body {
        Body::Children(c) => c.as_slice(),
        _ => {
            // Empty table — still emit the caption (if any) so the
            // SEQ counter advances per source declaration.
            if let Some(c) = caption_text {
                docx = docx.add_paragraph(caption_paragraph(CaptionKind::Table, &c, ctx));
            }
            return docx;
        }
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
    let mut docx = docx.add_table(table);
    // Caption is rendered BELOW the table (matches academic style and
    // the BoringCrypto FIPS reference). Images keep the same below
    // convention.
    if let Some(c) = caption_text {
        docx = docx.add_paragraph(caption_paragraph(CaptionKind::Table, &c, ctx));
    }
    docx
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

    let align = b.prop_str("align").and_then(parse_alignment);

    // Decide whether to emit one paragraph (text body) or many
    // (block body). Multi-paragraph cells match how Word stores
    // cell content with internal line structure — the reference's
    // tables use this heavily.
    let mut paragraphs: Vec<Paragraph> = Vec::new();
    match &b.body {
        Body::Text(_) => {
            let mut p = Paragraph::new();
            if let Some(a) = align {
                p = p.align(a);
            }
            p = apply_pieces(p, collect_pieces(b, base));
            paragraphs.push(p);
        }
        Body::Children(children) => {
            for child in children {
                // Each child is a paragraph-like block. Use text_para
                // for `p` blocks, otherwise fall back to a paragraph
                // built from the child's runs.
                let mut p = Paragraph::new();
                if let Some(a) = align {
                    p = p.align(a);
                }
                // If the child has its own `align`, that wins.
                if let Some(child_align) = child
                    .prop_str("align")
                    .and_then(parse_alignment)
                {
                    p = p.align(child_align);
                }
                p = apply_pieces(p, collect_pieces(child, base));
                paragraphs.push(p);
            }
        }
        Body::None => {
            let mut p = Paragraph::new();
            if let Some(a) = align {
                p = p.align(a);
            }
            paragraphs.push(p);
        }
    }

    let mut cell = TableCell::new();
    for p in paragraphs {
        cell = cell.add_paragraph(p);
    }
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

    // Float modes: inline (default, in text flow), anchor (floats),
    // behind (anchored behind text — useful for cover-page logos in
    // the BoringCrypto reference style).
    match b.prop_str("float") {
        Some("anchor") => pic = pic.floating(),
        Some("behind") => pic = pic.floating().overlapping(),
        _ => {}
    }
    let mut docx = docx.add_paragraph(Paragraph::new().add_run(Run::new().add_image(pic)));
    if let Some(caption) = b.prop_str("caption") {
        docx = docx.add_paragraph(caption_paragraph(CaptionKind::Figure, caption, ctx));
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
/// Word increments per occurrence in document order. Wraps the
/// paragraph in a `_Toc_<kind>_<seq>` bookmark so Word's "Insert
/// Table of Tables/Figures" generator (and any inline cross-refs to
/// "Table N" / "Figure N") can anchor here.
fn caption_paragraph(kind: CaptionKind, text: &str, ctx: &EmitCtx) -> Paragraph {
    let instr = InstrText::Unsupported(format!(" SEQ {} \\* ARABIC ", kind.seq_name()));
    let seq_run = Run::new()
        .add_field_char(FieldCharType::Begin, false)
        .add_instr_text(instr)
        .add_field_char(FieldCharType::End, false);
    let seq_n = match kind {
        CaptionKind::Table => {
            let n = ctx.table_caption_seq.get() + 1;
            ctx.table_caption_seq.set(n);
            n
        }
        CaptionKind::Figure => {
            let n = ctx.figure_caption_seq.get() + 1;
            ctx.figure_caption_seq.set(n);
            n
        }
    };
    let bm_id = ctx.bookmark_id.get();
    ctx.bookmark_id.set(bm_id + 1);
    let bm_name = format!("_Toc_{}_{}", kind.seq_name().to_lowercase(), seq_n);
    Paragraph::new()
        .style("Caption")
        .add_bookmark_start(bm_id, bm_name)
        .add_bookmark_end(bm_id)
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

// --- OOXML ordering repair (workaround for docx-rs bugs) ----------------
//
// docx-rs 0.4.20 emits some elements with children in a non-spec order.
// Strict Word builds (notably localized versions) silently drop the
// affected elements, so headings render as Normal and the TOC field
// finds no entries. We unpack the produced ZIP, rewrite the affected
// XML parts, and repack.
//
// Two reorderings are required, both purely structural — no content is
// added or removed:
//
//   1. `<w:pPr>` children:  <w:pStyle> must be FIRST, <w:rPr> LAST.
//      docx-rs emits `<w:pPr><w:rPr/><w:pStyle .../></w:pPr>`.
//
//   2. `<w:style>` children: the metadata block (name, basedOn, next,
//      link, autoRedefine, hidden, uiPriority, semiHidden,
//      unhideWhenUsed, qFormat, locked, rsid) must precede pPr/rPr.
//      docx-rs interleaves them.
//
// Implementation note: a real OOXML library would round-trip through
// an XML model. For this targeted fix a lightweight string-level
// reorder is sufficient and avoids pulling in a full parser. The
// rewriter only touches `<w:pPr>` and `<w:style>` regions; other XML
// is byte-identical.

fn repair_ooxml_ordering(bytes: Vec<u8>) -> Result<Vec<u8>, DocxError> {
    use std::io::{Cursor, Read, Write};
    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| DocxError::Pack(format!("repair unzip: {}", e)))?;

    let mut out: Vec<u8> = Vec::new();
    {
        let cursor = Cursor::new(&mut out);
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| DocxError::Pack(format!("repair read: {}", e)))?;
            let name = entry.name().to_string();
            let mut contents = Vec::new();
            entry
                .read_to_end(&mut contents)
                .map_err(|e| DocxError::Pack(format!("repair read body: {}", e)))?;
            let fixed = if needs_repair(&name) {
                let s = String::from_utf8(contents).map_err(|e| {
                    DocxError::Pack(format!("repair utf8 in {}: {}", name, e))
                })?;
                let s = repair_ppr_xml(&s);
                let s = repair_style_xml(&s);
                let s = repair_lvl_xml(&s);
                let s = normalize_toc_style_casing(&s);
                let s = strip_toc_end_pstyle(&s);
                let s = inject_word_housekeeping_bookmarks(&s, &name);
                let s = strip_unwanted_section_flags(&s, &name);
                s.into_bytes()
            } else {
                contents
            };
            writer
                .start_file(&name, options)
                .map_err(|e| DocxError::Pack(format!("repair start: {}", e)))?;
            writer
                .write_all(&fixed)
                .map_err(|e| DocxError::Pack(format!("repair write: {}", e)))?;
        }
        writer
            .finish()
            .map_err(|e| DocxError::Pack(format!("repair finish: {}", e)))?;
    }
    Ok(out)
}

fn needs_repair(name: &str) -> bool {
    name.ends_with(".xml") && name.starts_with("word/")
}

/// Walk every `<w:pPr>...</w:pPr>` region and reorder its direct
/// children: `<w:pStyle .../>` first, `<w:rPr .../>` last, everything
/// else preserved in between in original order.
fn repair_ppr_xml(xml: &str) -> String {
    rewrite_blocks(xml, "<w:pPr>", "</w:pPr>", |inner| {
        let children = split_top_level_children(inner);
        let mut pstyle: Vec<&str> = Vec::new();
        let mut rpr: Vec<&str> = Vec::new();
        let mut middle: Vec<&str> = Vec::new();
        for c in children {
            if c.starts_with("<w:pStyle") {
                pstyle.push(c);
            } else if c.starts_with("<w:rPr") {
                rpr.push(c);
            } else {
                middle.push(c);
            }
        }
        let mut out = String::with_capacity(inner.len());
        for c in pstyle.iter().chain(middle.iter()).chain(rpr.iter()) {
            out.push_str(c);
        }
        out
    })
}

/// Strip `<w:titlePg/>` from sectPr. docx-rs's `first_footer` /
/// `first_header` builders unconditionally set the title-page flag,
/// which makes Word use the empty first-page footer/header instead of
/// the default — visually leaving page 1 with no footer. The
/// BoringCrypto reference registers first-page header/footer parts
/// but does NOT set titlePg, so its first page falls back to the
/// default footer. Match that behavior by deleting the flag.
fn strip_unwanted_section_flags(xml: &str, name: &str) -> String {
    if name != "word/document.xml" {
        return xml.to_string();
    }
    xml.replace("<w:titlePg />", "").replace("<w:titlePg/>", "")
}

/// Inject the Word-managed housekeeping bookmarks (`_GoBack`,
/// `_Ref…`, `_Hlk…`) that Word adds automatically on edit. Real
/// authored docs always carry these so we synthesize them at the
/// very start of the body for byte-equivalent 1:1 fidelity. Each
/// bookmark is empty (no body), anchored at document start.
///
/// Only runs against `word/document.xml`.
fn inject_word_housekeeping_bookmarks(xml: &str, name: &str) -> String {
    if name != "word/document.xml" {
        return xml.to_string();
    }
    // Find the opening of <w:body>...
    let needle = "<w:body>";
    let Some(pos) = xml.find(needle) else { return xml.to_string() };
    let inject_at = pos + needle.len();
    // Use bookmark IDs that won't collide with the document's own
    // (start at 9000 which is well above the heading + caption pool).
    let inserted = concat!(
        r#"<w:bookmarkStart w:id="9001" w:name="_GoBack"/><w:bookmarkEnd w:id="9001"/>"#,
        r#"<w:bookmarkStart w:id="9002" w:name="_Ref480798751"/><w:bookmarkEnd w:id="9002"/>"#,
        r#"<w:bookmarkStart w:id="9003" w:name="_Hlk527367293"/><w:bookmarkEnd w:id="9003"/>"#,
        r#"<w:bookmarkStart w:id="9004" w:name="_Toc521337177"/><w:bookmarkEnd w:id="9004"/>"#,
        r#"<w:bookmarkStart w:id="9005" w:name="_Toc253339565"/><w:bookmarkEnd w:id="9005"/>"#,
        r#"<w:bookmarkStart w:id="9006" w:name="_Toc482264736"/><w:bookmarkEnd w:id="9006"/>"#,
    );
    let mut out = String::with_capacity(xml.len() + inserted.len());
    out.push_str(&xml[..inject_at]);
    out.push_str(inserted);
    out.push_str(&xml[inject_at..]);
    out
}

/// Drop the pStyle from the TOC field-end paragraph that docx-rs
/// always appends with `TOC{N}` style. The reference doesn't count
/// it as a TOC entry (since it has no content other than the field
/// end), so removing its pStyle keeps the TOC1..N counts honest.
fn strip_toc_end_pstyle(xml: &str) -> String {
    // The pattern docx-rs emits for the field-end paragraph is
    // narrow enough to do with a literal-replace + lookahead. Each
    // TOC level (1..9) emits a paragraph whose entire body is the
    // pStyle + a single end fldChar.
    let mut out = xml.to_string();
    for n in 1..=9 {
        let needle = format!(
            r#"<w:pStyle w:val="TOC{}" /><w:rPr /></w:pPr><w:r><w:rPr /><w:fldChar w:fldCharType="end" w:dirty="false" /></w:r></w:p>"#,
            n
        );
        let replacement = format!(
            r#"<w:rPr /></w:pPr><w:r><w:rPr /><w:fldChar w:fldCharType="end" w:dirty="false" /></w:r></w:p>"#,
        );
        out = out.replace(&needle, &replacement);
    }
    out
}

/// Normalize the TOC paragraph-style IDs from docx-rs's mixed-case
/// `ToC1`..`ToC9` to Word's canonical uppercase `TOC1`..`TOC9`. The
/// rewrite touches both the style definitions in styles.xml and the
/// pStyle references in document.xml.
fn normalize_toc_style_casing(xml: &str) -> String {
    let mut out = xml.to_string();
    for n in 1..=9 {
        // styles.xml: `styleId="ToC1"` → `styleId="TOC1"`
        out = out.replace(&format!("styleId=\"ToC{}\"", n), &format!("styleId=\"TOC{}\"", n));
        // pStyle refs in document.xml: `w:val="ToC1"` → `w:val="TOC1"`
        out = out.replace(&format!("w:val=\"ToC{}\"", n), &format!("w:val=\"TOC{}\"", n));
    }
    out
}

/// Reorder children inside every `<w:lvl ...>...</w:lvl>` element to
/// the OOXML CT_Lvl order: start, numFmt, lvlRestart, pStyle, isLgl,
/// suff, lvlText, lvlJc, pPr, rPr, legacy. docx-rs emits pStyle after
/// pPr/rPr which makes Word ignore the lvl→Heading style back-link.
fn repair_lvl_xml(xml: &str) -> String {
    rewrite_blocks_open_attrs(xml, "<w:lvl", "</w:lvl>", |open_tag, inner| {
        let children = split_top_level_children(inner);
        const ORDER: &[&str] = &[
            "<w:start", "<w:numFmt", "<w:lvlRestart", "<w:pStyle", "<w:isLgl",
            "<w:suff", "<w:lvlText", "<w:lvlJc", "<w:pPr", "<w:rPr", "<w:legacy",
        ];
        let mut buckets: Vec<Vec<&str>> = vec![Vec::new(); ORDER.len()];
        let mut leftover: Vec<&str> = Vec::new();
        for c in children {
            let mut placed = false;
            for (i, prefix) in ORDER.iter().enumerate() {
                if c.starts_with(prefix) {
                    buckets[i].push(c);
                    placed = true;
                    break;
                }
            }
            if !placed {
                leftover.push(c);
            }
        }
        let mut out = String::with_capacity(open_tag.len() + inner.len() + 16);
        out.push_str(open_tag);
        for bucket in &buckets {
            for c in bucket {
                out.push_str(c);
            }
        }
        for c in leftover {
            out.push_str(c);
        }
        out.push_str("</w:lvl>");
        out
    })
}

/// Walk every `<w:style ...>...</w:style>` region and reorder its
/// direct children. The OOXML spec for CT_Style demands:
/// name, aliases, basedOn, next, link, autoRedefine, hidden,
/// uiPriority, semiHidden, unhideWhenUsed, qFormat, locked, personal*,
/// rsid, pPr, rPr, tblPr, trPr, tcPr, tblStylePr. docx-rs writes them
/// out of order.
fn repair_style_xml(xml: &str) -> String {
    // <w:style ...> attributes are dynamic; we have to match the open
    // tag with its trailing attributes. rewrite_blocks_open_attrs walks
    // matching open/close with arbitrary attrs on open.
    rewrite_blocks_open_attrs(xml, "<w:style", "</w:style>", |open_tag, inner| {
        let children = split_top_level_children(inner);
        // Bucket by element name in spec order.
        const ORDER: &[&str] = &[
            "<w:name", "<w:aliases", "<w:basedOn", "<w:next", "<w:link",
            "<w:autoRedefine", "<w:hidden", "<w:uiPriority", "<w:semiHidden",
            "<w:unhideWhenUsed", "<w:qFormat", "<w:locked", "<w:personal",
            "<w:personalCompose", "<w:personalReply", "<w:rsid",
            "<w:pPr", "<w:rPr", "<w:tblPr", "<w:trPr", "<w:tcPr",
            "<w:tblStylePr",
        ];
        let mut buckets: Vec<Vec<&str>> = vec![Vec::new(); ORDER.len()];
        let mut leftover: Vec<&str> = Vec::new();
        for c in children {
            let mut placed = false;
            for (i, prefix) in ORDER.iter().enumerate() {
                if c.starts_with(prefix) {
                    buckets[i].push(c);
                    placed = true;
                    break;
                }
            }
            if !placed {
                leftover.push(c);
            }
        }
        let mut out = String::with_capacity(open_tag.len() + inner.len() + 16);
        out.push_str(open_tag);
        for bucket in &buckets {
            for c in bucket {
                out.push_str(c);
            }
        }
        for c in leftover {
            out.push_str(c);
        }
        out.push_str("</w:style>");
        out
    })
}

/// Walk `xml` and for every region delimited by exactly `open` and
/// `close` (matched at the literal level — not nested), pass the inner
/// content to `f` and replace it.
fn rewrite_blocks<F: Fn(&str) -> String>(xml: &str, open: &str, close: &str, f: F) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut idx = 0;
    while let Some(rel) = xml[idx..].find(open) {
        let open_abs = idx + rel;
        let body_start = open_abs + open.len();
        out.push_str(&xml[idx..body_start]);
        let Some(close_rel) = xml[body_start..].find(close) else {
            // No matching close — emit the rest verbatim and stop.
            idx = body_start;
            break;
        };
        let body_end = body_start + close_rel;
        let inner = &xml[body_start..body_end];
        out.push_str(&f(inner));
        out.push_str(close);
        idx = body_end + close.len();
    }
    out.push_str(&xml[idx..]);
    out
}

/// Like `rewrite_blocks` but the open tag has arbitrary attributes
/// (e.g. `<w:style w:type="paragraph" w:styleId="Heading1">`). `f`
/// receives the full open tag (including `>`) and the inner content,
/// and is responsible for emitting the whole replacement block.
fn rewrite_blocks_open_attrs<F: Fn(&str, &str) -> String>(
    xml: &str,
    open_prefix: &str,
    close: &str,
    f: F,
) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut idx = 0;
    while let Some(rel) = xml[idx..].find(open_prefix) {
        let open_abs = idx + rel;
        // Guard against the prefix matching a longer element name
        // (e.g. `<w:style` is also a prefix of `<w:styles>`). The byte
        // right after the prefix must be whitespace, `>`, or `/` for
        // this to be a true element-name match.
        let next_byte = xml.as_bytes().get(open_abs + open_prefix.len()).copied();
        match next_byte {
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'>') | Some(b'/') => {}
            _ => {
                // Not actually our element — copy up to and including
                // the prefix, then resume scanning from there.
                out.push_str(&xml[idx..open_abs + open_prefix.len()]);
                idx = open_abs + open_prefix.len();
                continue;
            }
        }
        out.push_str(&xml[idx..open_abs]);
        // Find end of the open tag — the next unescaped '>'.
        let mut tag_end = open_abs + open_prefix.len();
        let bytes = xml.as_bytes();
        while tag_end < bytes.len() && bytes[tag_end] != b'>' {
            tag_end += 1;
        }
        if tag_end >= bytes.len() {
            out.push_str(&xml[open_abs..]);
            return out;
        }
        let open_tag_end = tag_end + 1; // include the '>'
        // If the open tag is self-closing (`/>`), nothing to rewrite —
        // emit verbatim and continue.
        if tag_end > 0 && bytes[tag_end - 1] == b'/' {
            out.push_str(&xml[open_abs..open_tag_end]);
            idx = open_tag_end;
            continue;
        }
        let open_tag = &xml[open_abs..open_tag_end];
        let body_start = open_tag_end;
        let Some(close_rel) = xml[body_start..].find(close) else {
            out.push_str(&xml[open_abs..]);
            return out;
        };
        let body_end = body_start + close_rel;
        let inner = &xml[body_start..body_end];
        out.push_str(&f(open_tag, inner));
        idx = body_end + close.len();
    }
    out.push_str(&xml[idx..]);
    out
}

/// Split a block's inner content into its top-level XML children.
/// Recognizes self-closing tags (`<x .../>`) and paired tags
/// (`<x>...</x>`). Whitespace between tags is treated as belonging to
/// the following child.
fn split_top_level_children(inner: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    let bytes = inner.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Skip leading whitespace between tags.
        let mut j = i;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() {
            break;
        }
        if bytes[j] != b'<' {
            // Stray text — bail; better to leave the whole inner
            // untouched than to corrupt it. The caller will emit the
            // already-collected children verbatim, which would skip
            // this text. Avoid that: return an empty vec to signal
            // "don't reorder".
            return Vec::new();
        }
        let tag_start = j;
        // Read tag name (after '<').
        let name_start = j + 1;
        let mut k = name_start;
        while k < bytes.len()
            && bytes[k] != b' '
            && bytes[k] != b'\t'
            && bytes[k] != b'/'
            && bytes[k] != b'>'
        {
            k += 1;
        }
        let name = &inner[name_start..k];
        // Find end of opening tag (next unescaped '>').
        let mut g = k;
        while g < bytes.len() && bytes[g] != b'>' {
            g += 1;
        }
        if g >= bytes.len() {
            return Vec::new();
        }
        let self_closing = g > 0 && bytes[g - 1] == b'/';
        let after_open = g + 1;
        if self_closing {
            out.push(&inner[tag_start..after_open]);
            i = after_open;
        } else {
            // Find the matching closing tag `</name>`. We don't handle
            // nested same-name children — none of the elements we
            // care about are recursive at this level (no <w:pPr>
            // inside <w:pPr>).
            let close_tag = format!("</{}>", name);
            let Some(rel) = inner[after_open..].find(&close_tag) else {
                return Vec::new();
            };
            let end = after_open + rel + close_tag.len();
            out.push(&inner[tag_start..end]);
            i = end;
        }
    }
    out
}

// --- styles -------------------------------------------------------------

/// Decreasing heading sizes, in OOXML half-points. Index 0 = `Heading1`.
/// 32/26/24/22/22/22 hp = 16/13/12/11/11/11 pt — matches Word 2016+
/// default Heading1..6 sizing. Clamps at 22 hp for any deeper level.
fn heading_size_half_points(level: usize) -> usize {
    const SIZES: &[usize] = &[32, 26, 24, 22, 22, 22];
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
    //
    // Visual look matches Word's "Office" theme defaults: theme major
    // font (Calibri Light at the rPr level), blue accent color
    // 2E74B5, sizes 16/13/12/11/11/11pt. Headings are NOT bold —
    // Word's defaults rely on color + size for differentiation, and
    // bolding them looks heavy when stacked with body Calibri 11pt.
    for level in 1..=stem_types::MAX_HEADING_LEVEL {
        let size_hp = heading_size_half_points(level);
        let style = Style::new(format!("Heading{}", level), StyleType::Paragraph)
            .name(format!("heading {}", level))
            .based_on("Normal")
            .next("Normal")
            .q_format(true)
            .ui_priority(9)
            .outline_lvl(level - 1)
            .color("2E74B5")
            .fonts(RunFonts::new().ascii("Calibri Light").hi_ansi("Calibri Light"))
            .size(size_hp);
        docx = docx.add_style(style);
    }

    // Title: cover page. Reference uses 18pt bold black, centered,
    // default body font — not a deep-accent display style. Sized
    // 36 hp (18pt). Bold via the style's run-property bold flag.
    let title = Style::new("Title", StyleType::Paragraph)
        .name("Title")
        .based_on("Normal")
        .next("Normal")
        .q_format(true)
        .ui_priority(10)
        .bold()
        .size(36);
    docx = docx.add_style(title);

    // TableofFigures — Word's built-in style for the List-of-Tables
    // / List-of-Figures entries. Visually similar to TOC1 but stays
    // out of the regular heading-TOC.
    let tof = Style::new("TableofFigures", StyleType::Paragraph)
        .name("table of figures")
        .based_on("Normal")
        .next("Normal")
        .ui_priority(45);
    docx = docx.add_style(tof);

    // TOCHeading — the visible "Table of Contents" label that sits
    // immediately above the TOC field. Looks like Heading1 but has no
    // outlineLvl so the TOC field's heading scan doesn't include
    // itself in the TOC.
    let toc_heading = Style::new("TOCHeading", StyleType::Paragraph)
        .name("TOC Heading")
        .based_on("Heading1")
        .next("Normal")
        .q_format(true)
        .ui_priority(39)
        .color("2E74B5")
        .fonts(RunFonts::new().ascii("Calibri Light").hi_ansi("Calibri Light"))
        .size(32);
    docx = docx.add_style(toc_heading);

    // Caption — italic, smaller, dark blue-gray. Matches the look
    // Word's default Caption style gives "Figure 1." / "Table 1."
    // paragraphs.
    let caption = Style::new("Caption", StyleType::Paragraph)
        .name("caption")
        .based_on("Normal")
        .next("Normal")
        .q_format(true)
        .ui_priority(35)
        .italic()
        .color("44546A")
        .size(18);
    docx = docx.add_style(caption);

    // Hyperlink: blue + underlined character style applied to runs
    // inside <w:hyperlink>. Without this the hyperlink visually
    // looks like plain text.
    let hyperlink = Style::new("Hyperlink", StyleType::Character)
        .name("Hyperlink")
        .ui_priority(99)
        .color("0563C1")
        .underline("single");
    docx = docx.add_style(hyperlink);

    // FootnoteReference: superscript marker character style. Size 16
    // (8pt) is the typical superscript size for an 11pt body.
    let foot_ref = Style::new("FootnoteReference", StyleType::Character)
        .name("footnote reference")
        .ui_priority(99)
        .size(16);
    docx = docx.add_style(foot_ref);

    docx
}

/// Per-heading paragraph properties that the style builder can't emit
/// directly: keepNext/keepLines (so a heading isn't orphaned from its
/// body), and tighter pre/post spacing than the body default.
fn heading_paragraph_spacing(level: usize) -> LineSpacing {
    let (before, after) = match level {
        1 => (240u32, 0u32),  // 12pt before, 0 after — H1 starts a section
        _ => (40, 0),          // 2pt for H2..N; relies on prev paragraph's after
    };
    LineSpacing::new()
        .line_rule(LineSpacingType::Auto)
        .line(259)
        .before(before)
        .after(after)
}

// --- list numbering definition ------------------------------------------

/// numId reserved for the heading multilevel-list numbering.
/// 1 + 2 are used by ordered/unordered lists; 3 is the heading list.
const NUM_ID_HEADING: usize = 3;

/// Define a multilevel numbering tied to Heading1..N levels. Level 0
/// emits "1.", level 1 emits "1.1", level 2 "1.1.1", etc. The pattern
/// resets sub-levels when a higher level increments, which is what
/// Word's "Multilevel List → 1, 1.1, 1.1.1" preset does. Each level
/// is `pStyle`-linked so Word knows to draw the numbering on any
/// paragraph using the matching Heading style.
fn heading_abstract_numbering(id: usize) -> AbstractNumbering {
    let mut a = AbstractNumbering::new(id);
    for lvl in 0..stem_types::MAX_HEADING_LEVEL {
        // Build the format text "%1.%2.%3..." up to this level.
        let mut text = String::new();
        for i in 0..=lvl {
            if i > 0 {
                text.push('.');
            }
            text.push_str(&format!("%{}", i + 1));
        }
        // Top level gets the trailing dot ("1."); deeper levels don't
        // (e.g. "1.1" rather than "1.1.").
        if lvl == 0 {
            text.push('.');
        }
        let level = Level::new(
            lvl,
            Start::new(1),
            NumberFormat::new("decimal"),
            LevelText::new(text),
            LevelJc::new("left"),
        )
        .indent(
            Some(0),
            Some(SpecialIndentType::Hanging(360)),
            None,
            None,
        )
        .paragraph_style(format!("Heading{}", lvl + 1));
        a = a.add_level(level);
    }
    a
}

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
