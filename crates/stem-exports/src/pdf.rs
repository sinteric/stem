//! PDF exporter — native, via `printpdf`. No shellout, no headless
//! browser, no Typst dep.
//!
//! MVP scope: headings (h1..h6, larger font), paragraphs (with naive
//! word wrap), ordered/unordered lists, blockquote (indented + italic),
//! code blocks (monospace, no syntax highlighting), inline emphasis
//! (italic/bold via font swap), horizontal rule.
//!
//! Out of MVP scope (deliberate): images, tables, sheets, multi-column
//! layouts, links/cross-refs, footnotes, true font metrics (we
//! approximate average glyph width). Latin scripts only — built-in PDF
//! base-14 fonts cover Latin-1; CJK requires bundling a CJK font, which
//! is a follow-up.

use std::fmt::Write as _;

use printpdf::{
    BuiltinFont, Color, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Pt,
    Rgb, TextItem,
};
use stem_core::ast::{Block, Body, Document, TextPiece};
use stem_core::theme::Theme;
use stem_core::Exporter;
use thiserror::Error;

#[derive(Default)]
pub struct PdfExporter;

impl PdfExporter {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Error)]
pub enum PdfError {
    // Future: surface print warnings, font issues, etc. For MVP there's
    // nothing that fails on the happy path.
    #[error("pdf export failed: {0}")]
    Other(String),
}

impl Exporter for PdfExporter {
    type Output = Vec<u8>;
    type Error = PdfError;
    fn export(&self, doc: &Document, _theme: &Theme) -> Result<Vec<u8>, PdfError> {
        let cooked = stem_parser::cook_document(doc);
        let title = cooked.metadata.get_str("title").unwrap_or("Stem document");

        let mut layout = Layout::new();
        for block in &cooked.blocks {
            layout.emit_block(block);
        }
        layout.finish_page();

        let mut pdf = PdfDocument::new(title);
        pdf.with_pages(layout.pages);
        let mut warnings = Vec::new();
        Ok(pdf.save(&PdfSaveOptions::default(), &mut warnings))
    }
}

// ---------------------------------------------------------------------------

// A4 page in mm.
const PAGE_W_MM: f32 = 210.0;
const PAGE_H_MM: f32 = 297.0;
const MARGIN_MM: f32 = 20.0;

// mm → pt (PDF native unit).
const MM_TO_PT: f32 = 2.834645669;

// Approximate average glyph width as fraction of font size, used for
// naive word wrap. Real font metrics would replace this; for MVP this
// is good enough for Helvetica/Times body text.
const AVG_GLYPH_RATIO: f32 = 0.50;

// Font sizes (pt) per element class.
const BODY_PT: f32 = 11.0;
const H1_PT: f32 = 24.0;
const H2_PT: f32 = 20.0;
const H3_PT: f32 = 16.0;
const H4_PT: f32 = 14.0;
const H5_PT: f32 = 12.0;
const H6_PT: f32 = 11.0;
const CODE_PT: f32 = 10.0;

// Line height multiplier on the current font size.
const LINE_HEIGHT_RATIO: f32 = 1.4;

// Vertical gap after each block, expressed in em (multiples of the
// block's font size).
const BLOCK_GAP_EM: f32 = 0.6;

struct Layout {
    pages: Vec<PdfPage>,
    /// Operations for the page currently under construction.
    current_ops: Vec<Op>,
    /// Current Y cursor, measured DOWN from the top margin (page-space,
    /// not PDF-space).
    cursor_y_from_top: f32,
}

impl Layout {
    fn new() -> Self {
        let mut s = Self {
            pages: Vec::new(),
            current_ops: Vec::new(),
            cursor_y_from_top: 0.0,
        };
        s.start_page();
        s
    }

    fn start_page(&mut self) {
        self.current_ops.clear();
        self.current_ops.push(Op::StartTextSection);
        self.cursor_y_from_top = 0.0;
    }

    fn finish_page(&mut self) {
        if self.current_ops.is_empty() {
            return;
        }
        let mut ops = std::mem::take(&mut self.current_ops);
        ops.push(Op::EndTextSection);
        self.pages
            .push(PdfPage::new(Mm(PAGE_W_MM), Mm(PAGE_H_MM), ops));
    }

    fn ensure_room(&mut self, needed_pt: f32) {
        let available_pt = (PAGE_H_MM - 2.0 * MARGIN_MM) * MM_TO_PT - self.cursor_y_from_top;
        if needed_pt > available_pt {
            self.finish_page();
            self.start_page();
        }
    }

    /// Position the cursor for the next line at the current y-from-top,
    /// then advance the cursor by `line_height_pt`.
    fn write_line(&mut self, text: &str, font: PdfFontHandle, size_pt: f32) {
        let line_h = size_pt * LINE_HEIGHT_RATIO;
        self.ensure_room(line_h);
        // PDF origin is bottom-left. Convert "y from top" to PDF y.
        let y_pdf_pt = (PAGE_H_MM - MARGIN_MM) * MM_TO_PT - self.cursor_y_from_top - size_pt;
        let pos = Point {
            x: Mm(MARGIN_MM).into(),
            y: Pt(y_pdf_pt).into(),
        };
        self.current_ops.push(Op::SetFont { font, size: Pt(size_pt) });
        self.current_ops.push(Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.08,
                g: 0.10,
                b: 0.13,
                icc_profile: None,
            }),
        });
        self.current_ops.push(Op::SetTextCursor { pos });
        self.current_ops.push(Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        });
        self.cursor_y_from_top += line_h;
    }

    fn block_gap(&mut self, size_pt: f32) {
        self.cursor_y_from_top += size_pt * BLOCK_GAP_EM;
    }

    fn emit_block(&mut self, b: &Block) {
        match b.name.as_str() {
            "h1" => self.emit_heading(b, H1_PT, BuiltinFont::HelveticaBold),
            "h2" => self.emit_heading(b, H2_PT, BuiltinFont::HelveticaBold),
            "h3" => self.emit_heading(b, H3_PT, BuiltinFont::HelveticaBold),
            "h4" => self.emit_heading(b, H4_PT, BuiltinFont::HelveticaBold),
            "h5" => self.emit_heading(b, H5_PT, BuiltinFont::HelveticaBold),
            "h6" => self.emit_heading(b, H6_PT, BuiltinFont::HelveticaBold),
            "p" => self.emit_paragraph(b, BODY_PT, BuiltinFont::Helvetica),
            "blockquote" => self.emit_paragraph(b, BODY_PT, BuiltinFont::HelveticaOblique),
            "code" => self.emit_code_block(b),
            "ol" | "ul" => self.emit_list(b),
            "hr" => self.emit_hr(),
            _ => {
                // Unknown blocks → render the plain text body, if any,
                // so content isn't lost.
                self.emit_paragraph(b, BODY_PT, BuiltinFont::Helvetica);
            }
        }
    }

    fn emit_heading(&mut self, b: &Block, size_pt: f32, font: BuiltinFont) {
        let text = flatten_text(b);
        if text.is_empty() {
            return;
        }
        for line in wrap_lines(&text, size_pt, content_width_pt()) {
            self.write_line(&line, PdfFontHandle::Builtin(font), size_pt);
        }
        self.block_gap(size_pt);
    }

    fn emit_paragraph(&mut self, b: &Block, size_pt: f32, font: BuiltinFont) {
        let text = flatten_text(b);
        if text.is_empty() {
            return;
        }
        for line in wrap_lines(&text, size_pt, content_width_pt()) {
            self.write_line(&line, PdfFontHandle::Builtin(font), size_pt);
        }
        self.block_gap(size_pt);
    }

    fn emit_code_block(&mut self, b: &Block) {
        let text = b.plain_text().unwrap_or_default();
        for line in text.lines() {
            self.write_line(line, PdfFontHandle::Builtin(BuiltinFont::Courier), CODE_PT);
        }
        self.block_gap(CODE_PT);
    }

    fn emit_list(&mut self, b: &Block) {
        let ordered = b.name == "ol";
        let start: usize = b
            .prop_str("start")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        if let Body::Children(items) = &b.body {
            for (i, item) in items.iter().enumerate() {
                let marker = if ordered {
                    format!("{}. ", start + i)
                } else {
                    "• ".to_string()
                };
                let body_text = flatten_text(item);
                let full = format!("{}{}", marker, body_text);
                for line in wrap_lines(&full, BODY_PT, content_width_pt()) {
                    self.write_line(
                        &line,
                        PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                        BODY_PT,
                    );
                }
            }
            self.block_gap(BODY_PT);
        }
    }

    fn emit_hr(&mut self) {
        // No drawing primitives yet in MVP — just leave a blank line as
        // a visual break. Future: emit Op::DrawLine.
        self.block_gap(BODY_PT * 1.5);
    }
}

// ---------------------------------------------------------------------------

fn content_width_pt() -> f32 {
    (PAGE_W_MM - 2.0 * MARGIN_MM) * MM_TO_PT
}

/// Naive average-glyph-width word wrap. Returns lines that should fit
/// within `width_pt` at the given font size. Words longer than the
/// content width are emitted on their own line and overflow silently
/// (true measurement / hyphenation is a real-typography concern).
fn wrap_lines(text: &str, size_pt: f32, width_pt: f32) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let avg_w = size_pt * AVG_GLYPH_RATIO;
    let max_chars = (width_pt / avg_w).floor() as usize;
    if max_chars == 0 {
        return vec![text.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate_len = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if candidate_len > max_chars && !current.is_empty() {
            out.push(std::mem::take(&mut current));
            current.push_str(word);
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Concatenate text body pieces into a single string, dropping inline
/// element formatting (MVP renders all inline content as plain text).
fn flatten_text(b: &Block) -> String {
    let mut out = String::new();
    if let Body::Text(pieces) = &b.body {
        for piece in pieces {
            match piece {
                TextPiece::Literal { text, .. } => out.push_str(text),
                TextPiece::Inline(inline) => {
                    if let Some(t) = inline.plain_text() {
                        let _ = write!(out, "{}", t);
                    }
                }
            }
        }
    }
    out
}
