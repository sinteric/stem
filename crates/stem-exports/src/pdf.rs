//! PDF exporter — native, via `printpdf`. No shellout, no headless
//! browser, no Typst dep.
//!
//! Scope:
//! - Headings h1..h6 (larger HelveticaBold, or the configured custom
//!   font if present)
//! - Paragraphs with word wrap using real font glyph-width metrics
//! - Lists (ordered/unordered)
//! - Blockquote (oblique)
//! - Code blocks (Courier monospace, no syntax highlighting)
//! - Horizontal rule (just a gap; drawing primitives TODO)
//! - Inline italic and bold runs (`@text[style:italic]`,
//!   `@text[weight:bold]`) — Latin built-in fonts only; with a custom
//!   font configured, runs all use that single font.
//!
//! ## Custom fonts (CJK and others)
//!
//! Built-in PDF base-14 fonts cover Latin-1 only. To render CJK or any
//! other non-Latin text, the embedder must supply font bytes.
//!
//! The simplest path is one font for everything:
//!
//! ```ignore
//! let bytes = std::fs::read("/path/to/NotoSansKR-Regular.ttf")?;
//! let exporter = PdfExporter::new().with_font(bytes);
//! ```
//!
//! For documents that use bold or italic emphasis, pass a full family
//! so heading weight survives:
//!
//! ```ignore
//! let exporter = PdfExporter::new().with_font_family(
//!     std::fs::read("NotoSansKR-Regular.ttf")?,
//!     Some(std::fs::read("NotoSansKR-Bold.ttf")?),
//!     None, // no italic for many CJK fonts; falls back to regular
//!     None,
//! );
//! ```
//!
//! Load fonts however the embedder prefers — the simplest pattern is
//! `std::fs::read(path)?` at the call site:
//!
//! ```ignore
//! let exporter = PdfExporter::new().with_font(std::fs::read(path)?);
//! ```
//!
//! Where to get a font: any open TTF/OTF works. Common choices for
//! Korean: Noto Sans KR (Google Fonts), NanumGothic (Naver), Pretendard.
//! For pan-CJK: Noto Sans CJK (much larger). System fonts on macOS like
//! `/System/Library/Fonts/AppleSDGothicNeo.ttc` also work (printpdf
//! reads font index 0 from a TTC).
//!
//! Out of scope: images, tables, sheets, links, footnotes, headers/
//! footers, hyphenation, justified text.

use std::fmt::Write as _;

use printpdf::{
    BuiltinFont, Color, FontId, Mm, Op, ParsedFont, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, Point, Pt, Rgb, TextItem,
};
use stem_core::ast::{Block, Body, Document, TextPiece};
use stem_core::theme::Theme;
use stem_core::Exporter;
use thiserror::Error;

/// Exporter for PDF output. Configure with [`with_font`](Self::with_font)
/// to provide a font for non-Latin text (e.g. Noto Sans CJK).
#[derive(Default)]
pub struct PdfExporter {
    family: CustomFamily,
}

#[derive(Default, Clone)]
struct CustomFamily {
    regular: Option<Vec<u8>>,
    bold: Option<Vec<u8>>,
    italic: Option<Vec<u8>>,
    bold_italic: Option<Vec<u8>>,
}

impl PdfExporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Use the given font for all body, heading, and inline text. The
    /// font replaces the built-in Latin family entirely; without bold
    /// or italic variants supplied separately (see
    /// [`with_font_family`](Self::with_font_family)), runs marked bold
    /// or italic fall back to the same regular font.
    ///
    /// Required for CJK or any other non-Latin text — the built-in PDF
    /// base-14 fonts only cover Latin-1.
    pub fn with_font(mut self, bytes: Vec<u8>) -> Self {
        self.family.regular = Some(bytes);
        self
    }

    /// Provide a full font family. Optional variants null out to the
    /// regular font (so a CJK font with no italic still renders italic
    /// runs in the regular weight, instead of switching to the built-in
    /// Helvetica which lacks CJK glyphs).
    pub fn with_font_family(
        mut self,
        regular: Vec<u8>,
        bold: Option<Vec<u8>>,
        italic: Option<Vec<u8>>,
        bold_italic: Option<Vec<u8>>,
    ) -> Self {
        self.family = CustomFamily {
            regular: Some(regular),
            bold,
            italic,
            bold_italic,
        };
        self
    }

}

#[derive(Debug, Error)]
pub enum PdfError {
    #[error("pdf export failed: {0}")]
    Other(String),
}

impl Exporter for PdfExporter {
    type Output = Vec<u8>;
    type Error = PdfError;
    fn export(&self, doc: &Document, _theme: &Theme) -> Result<Vec<u8>, PdfError> {
        let cooked = stem_parser::cook_document(doc);
        let title = cooked.metadata.get_str("title").unwrap_or("Stem document");

        let mut pdf = PdfDocument::new(title);

        // Parse each custom variant once. ParsedFont gives us metrics
        // for real word wrap (glyph widths) and a FontId for embedding.
        let parse_variant = |bytes: &Option<Vec<u8>>| -> Option<ParsedFont> {
            bytes
                .as_ref()
                .and_then(|b| ParsedFont::from_bytes(b, 0, &mut Vec::new()))
        };
        let custom_regular = parse_variant(&self.family.regular);
        let custom_bold = parse_variant(&self.family.bold);
        let custom_italic = parse_variant(&self.family.italic);
        let custom_bold_italic = parse_variant(&self.family.bold_italic);

        let mut add_id = |f: &Option<ParsedFont>| -> Option<FontId> {
            f.as_ref().map(|p| pdf.add_font(p))
        };
        let custom_regular_id = add_id(&custom_regular);
        let custom_bold_id = add_id(&custom_bold);
        let custom_italic_id = add_id(&custom_italic);
        let custom_bold_italic_id = add_id(&custom_bold_italic);

        let helvetica = BuiltinFont::Helvetica.get_parsed_font();
        let helvetica_bold = BuiltinFont::HelveticaBold.get_parsed_font();
        let helvetica_oblique = BuiltinFont::HelveticaOblique.get_parsed_font();
        let courier = BuiltinFont::Courier.get_parsed_font();

        let fonts = Fonts {
            custom_regular: custom_regular.as_ref(),
            custom_regular_id: custom_regular_id.as_ref(),
            custom_bold: custom_bold.as_ref(),
            custom_bold_id: custom_bold_id.as_ref(),
            custom_italic: custom_italic.as_ref(),
            custom_italic_id: custom_italic_id.as_ref(),
            custom_bold_italic: custom_bold_italic.as_ref(),
            custom_bold_italic_id: custom_bold_italic_id.as_ref(),
            helvetica: helvetica.as_ref(),
            helvetica_bold: helvetica_bold.as_ref(),
            helvetica_oblique: helvetica_oblique.as_ref(),
            courier: courier.as_ref(),
        };

        let mut layout = Layout::new(fonts);
        for block in &cooked.blocks {
            layout.emit_block(block);
        }
        layout.finish_page();

        pdf.with_pages(layout.pages);
        let mut warnings = Vec::new();
        Ok(pdf.save(&PdfSaveOptions::default(), &mut warnings))
    }
}

// ---------------------------------------------------------------------------

const PAGE_W_MM: f32 = 210.0;
const PAGE_H_MM: f32 = 297.0;
const MARGIN_MM: f32 = 20.0;
const MM_TO_PT: f32 = 2.834645669;

const BODY_PT: f32 = 11.0;
const H1_PT: f32 = 24.0;
const H2_PT: f32 = 20.0;
const H3_PT: f32 = 16.0;
const H4_PT: f32 = 14.0;
const H5_PT: f32 = 12.0;
const H6_PT: f32 = 11.0;
const CODE_PT: f32 = 10.0;

const LINE_HEIGHT_RATIO: f32 = 1.4;
const BLOCK_GAP_EM: f32 = 0.6;

/// Identifies which font role a run wants. The actual font selected at
/// emit time depends on whether a custom font is configured.
#[derive(Clone, Copy, Debug, PartialEq)]
enum FontRole {
    Body,
    Bold,
    Italic,
    BoldItalic,
    Monospace,
}

#[derive(Clone, Copy)]
struct Fonts<'a> {
    custom_regular: Option<&'a ParsedFont>,
    custom_regular_id: Option<&'a FontId>,
    custom_bold: Option<&'a ParsedFont>,
    custom_bold_id: Option<&'a FontId>,
    custom_italic: Option<&'a ParsedFont>,
    custom_italic_id: Option<&'a FontId>,
    custom_bold_italic: Option<&'a ParsedFont>,
    custom_bold_italic_id: Option<&'a FontId>,
    helvetica: Option<&'a ParsedFont>,
    helvetica_bold: Option<&'a ParsedFont>,
    helvetica_oblique: Option<&'a ParsedFont>,
    courier: Option<&'a ParsedFont>,
}

impl<'a> Fonts<'a> {
    /// Resolve a FontRole to (handle, metrics). The cascade for a
    /// custom family is:
    ///   role-specific variant → regular variant → built-in equivalent
    ///
    /// So a CJK font with no italic still renders italic runs in the
    /// regular CJK weight (rather than switching to Helvetica which
    /// has no CJK glyphs). Code/Monospace always uses Courier — there
    /// is no monospace variant in the custom-family API by design.
    fn resolve(&self, role: FontRole) -> (PdfFontHandle, Option<&'a ParsedFont>) {
        if role == FontRole::Monospace {
            return (
                PdfFontHandle::Builtin(BuiltinFont::Courier),
                self.courier,
            );
        }
        let custom = match role {
            FontRole::Body => (self.custom_regular_id, self.custom_regular),
            FontRole::Bold => (
                self.custom_bold_id.or(self.custom_regular_id),
                self.custom_bold.or(self.custom_regular),
            ),
            FontRole::Italic => (
                self.custom_italic_id.or(self.custom_regular_id),
                self.custom_italic.or(self.custom_regular),
            ),
            FontRole::BoldItalic => (
                self.custom_bold_italic_id
                    .or(self.custom_bold_id)
                    .or(self.custom_italic_id)
                    .or(self.custom_regular_id),
                self.custom_bold_italic
                    .or(self.custom_bold)
                    .or(self.custom_italic)
                    .or(self.custom_regular),
            ),
            FontRole::Monospace => unreachable!(),
        };
        if let (Some(id), parsed) = custom {
            return (PdfFontHandle::External(id.clone()), parsed);
        }
        match role {
            FontRole::Body => (
                PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                self.helvetica,
            ),
            FontRole::Bold => (
                PdfFontHandle::Builtin(BuiltinFont::HelveticaBold),
                self.helvetica_bold,
            ),
            FontRole::Italic => (
                PdfFontHandle::Builtin(BuiltinFont::HelveticaOblique),
                self.helvetica_oblique,
            ),
            FontRole::BoldItalic => (
                PdfFontHandle::Builtin(BuiltinFont::HelveticaBoldOblique),
                self.helvetica_bold, // metrics close enough for wrap
            ),
            FontRole::Monospace => unreachable!(),
        }
    }
}

struct Layout<'a> {
    fonts: Fonts<'a>,
    pages: Vec<PdfPage>,
    current_ops: Vec<Op>,
    cursor_y_from_top: f32,
}

impl<'a> Layout<'a> {
    fn new(fonts: Fonts<'a>) -> Self {
        let mut s = Self {
            fonts,
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

    /// Emit one line containing a sequence of styled runs. Each run is
    /// `(text, role)`. Runs share a line; the font swaps mid-line.
    fn write_runs(&mut self, runs: &[(String, FontRole)], size_pt: f32) {
        let line_h = size_pt * LINE_HEIGHT_RATIO;
        self.ensure_room(line_h);
        let y_pdf_pt = (PAGE_H_MM - MARGIN_MM) * MM_TO_PT - self.cursor_y_from_top - size_pt;
        let mut x_pt = MARGIN_MM * MM_TO_PT;
        // Set initial cursor; subsequent runs continue from where the
        // last text-show left off (PDF's Tj advances the cursor for us).
        self.current_ops.push(Op::SetTextCursor {
            pos: Point {
                x: Pt(x_pt).into(),
                y: Pt(y_pdf_pt).into(),
            },
        });
        let _ = &mut x_pt; // x_pt isn't recomputed; we trust PDF advance
        self.current_ops.push(Op::SetFillColor {
            col: Color::Rgb(Rgb {
                r: 0.08,
                g: 0.10,
                b: 0.13,
                icc_profile: None,
            }),
        });
        for (text, role) in runs {
            let (handle, _) = self.fonts.resolve(*role);
            self.current_ops.push(Op::SetFont {
                font: handle,
                size: Pt(size_pt),
            });
            self.current_ops.push(Op::ShowText {
                items: vec![TextItem::Text(text.clone())],
            });
        }
        self.cursor_y_from_top += line_h;
    }

    fn block_gap(&mut self, size_pt: f32) {
        self.cursor_y_from_top += size_pt * BLOCK_GAP_EM;
    }

    fn emit_block(&mut self, b: &Block) {
        match b.name.as_str() {
            "h1" => self.emit_heading(b, H1_PT),
            "h2" => self.emit_heading(b, H2_PT),
            "h3" => self.emit_heading(b, H3_PT),
            "h4" => self.emit_heading(b, H4_PT),
            "h5" => self.emit_heading(b, H5_PT),
            "h6" => self.emit_heading(b, H6_PT),
            "p" => self.emit_paragraph(b, BODY_PT, FontRole::Body),
            "blockquote" => self.emit_paragraph(b, BODY_PT, FontRole::Italic),
            "code" => self.emit_code_block(b),
            "ol" | "ul" => self.emit_list(b),
            "hr" => self.block_gap(BODY_PT * 1.5),
            _ => self.emit_paragraph(b, BODY_PT, FontRole::Body),
        }
    }

    fn emit_heading(&mut self, b: &Block, size_pt: f32) {
        let runs = collect_runs(b, FontRole::Bold);
        self.emit_runs_wrapped(&runs, size_pt);
        self.block_gap(size_pt);
    }

    fn emit_paragraph(&mut self, b: &Block, size_pt: f32, base_role: FontRole) {
        let runs = collect_runs(b, base_role);
        self.emit_runs_wrapped(&runs, size_pt);
        self.block_gap(size_pt);
    }

    fn emit_code_block(&mut self, b: &Block) {
        let text = b.plain_text().unwrap_or_default();
        for line in text.lines() {
            self.write_runs(&[(line.to_string(), FontRole::Monospace)], CODE_PT);
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
                let mut runs = vec![(marker, FontRole::Body)];
                runs.extend(collect_runs(item, FontRole::Body));
                self.emit_runs_wrapped(&runs, BODY_PT);
            }
            self.block_gap(BODY_PT);
        }
    }

    /// Take a list of styled runs (the full paragraph) and wrap them to
    /// the content width. Each output line gets `write_runs` called on
    /// it with its own slice of runs.
    fn emit_runs_wrapped(&mut self, runs: &[(String, FontRole)], size_pt: f32) {
        if runs.is_empty() {
            return;
        }
        let width_pt = content_width_pt();
        let mut current_line: Vec<(String, FontRole)> = Vec::new();
        let mut current_width = 0.0_f32;

        for (text, role) in runs {
            let (_, parsed) = self.fonts.resolve(*role);
            // Walk word boundaries inside this run, building lines.
            // Whitespace serves as wrap point.
            let mut word_buf = String::new();
            let mut chars = text.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\n' || c.is_whitespace() {
                    // Flush any pending word.
                    if !word_buf.is_empty() {
                        self.consume_word(
                            &word_buf,
                            *role,
                            parsed,
                            size_pt,
                            width_pt,
                            &mut current_line,
                            &mut current_width,
                        );
                        word_buf.clear();
                    }
                    if c == '\n' {
                        // Hard break: flush line.
                        if !current_line.is_empty() {
                            self.write_runs(&current_line, size_pt);
                            current_line.clear();
                            current_width = 0.0;
                        }
                    } else {
                        // Soft space: append if line non-empty.
                        let space_w = measure_str(parsed, " ", size_pt);
                        if !current_line.is_empty() {
                            current_line.push((" ".to_string(), *role));
                            current_width += space_w;
                        }
                    }
                } else {
                    word_buf.push(c);
                }
            }
            if !word_buf.is_empty() {
                self.consume_word(
                    &word_buf,
                    *role,
                    parsed,
                    size_pt,
                    width_pt,
                    &mut current_line,
                    &mut current_width,
                );
            }
        }
        if !current_line.is_empty() {
            self.write_runs(&current_line, size_pt);
        }
    }

    fn consume_word(
        &mut self,
        word: &str,
        role: FontRole,
        parsed: Option<&'a ParsedFont>,
        size_pt: f32,
        width_pt: f32,
        current_line: &mut Vec<(String, FontRole)>,
        current_width: &mut f32,
    ) {
        let w = measure_str(parsed, word, size_pt);
        if *current_width + w > width_pt && !current_line.is_empty() {
            // Wrap: emit current line, start fresh with this word.
            // Trim a trailing space if any.
            while matches!(current_line.last(), Some((s, _)) if s == " ") {
                current_line.pop();
            }
            self.write_runs(current_line, size_pt);
            current_line.clear();
            *current_width = 0.0;
        }
        current_line.push((word.to_string(), role));
        *current_width += w;
    }
}

// ---------------------------------------------------------------------------

fn content_width_pt() -> f32 {
    (PAGE_W_MM - 2.0 * MARGIN_MM) * MM_TO_PT
}

/// Measure a string's width in PDF points using real glyph metrics
/// from the font (units-per-em assumed 1000, the PDF/TrueType default).
/// Falls back to average glyph estimate when the font is unavailable
/// or characters are unmapped.
fn measure_str(font: Option<&ParsedFont>, text: &str, size_pt: f32) -> f32 {
    let Some(f) = font else {
        return text.chars().count() as f32 * size_pt * 0.5;
    };
    let mut units = 0u32;
    for ch in text.chars() {
        if let Some(gid) = f.lookup_glyph_index(ch as u32) {
            let advance = f.get_horizontal_advance(gid);
            if advance > 0 {
                units += advance as u32;
                continue;
            }
        }
        // Missing glyph or no advance recorded: assume average half-em.
        units += 500;
    }
    (units as f32 / 1000.0) * size_pt
}

/// Walk a block's body and collect (text, role) runs. Inline @text
/// elements with `weight:bold` or `style:italic` switch role; everything
/// else uses `base_role`. Whitespace is preserved.
fn collect_runs(b: &Block, base_role: FontRole) -> Vec<(String, FontRole)> {
    let mut out: Vec<(String, FontRole)> = Vec::new();
    if let Body::Text(pieces) = &b.body {
        for piece in pieces {
            match piece {
                TextPiece::Literal { text, .. } => {
                    out.push((text.clone(), base_role));
                }
                TextPiece::Inline(inline) => {
                    let role = inline_role(inline, base_role);
                    let mut s = String::new();
                    if let Some(t) = inline.plain_text() {
                        let _ = write!(s, "{}", t);
                    }
                    if !s.is_empty() {
                        out.push((s, role));
                    }
                }
            }
        }
    }
    out
}

fn inline_role(b: &Block, base_role: FontRole) -> FontRole {
    if b.name != "text" {
        return base_role;
    }
    let bold = b.prop_str("weight") == Some("bold");
    let italic = b.prop_str("style") == Some("italic");
    match (bold, italic) {
        (true, true) => FontRole::BoldItalic,
        (true, false) => FontRole::Bold,
        (false, true) => FontRole::Italic,
        (false, false) => base_role,
    }
}
