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
    AbstractNumbering, Docx, IndentLevel, Level, LevelJc, LevelText, NumberFormat, Numbering,
    NumberingId, Paragraph, Run, RunFonts, SpecialIndentType, Start,
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
    fn export(&self, doc: &Document, _theme: &Theme) -> Result<Vec<u8>, DocxError> {
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
            out = emit_block(out, block, 0);
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

fn emit_block(docx: Docx, b: &Block, _depth: usize) -> Docx {
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
