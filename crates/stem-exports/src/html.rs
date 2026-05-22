//! HTML exporter.
//!
//! Walks a `stem_core::ast::Document` and produces HTML. Per-element
//! render functions live in the [`elements`] submodule; this file owns
//! the document walker and the generic fallback for unknown elements.

pub mod elements;

use std::fmt::Write;

use stem_core::ast::*;
use stem_core::theme::Theme;
use stem_core::Exporter;

#[derive(Default)]
pub struct HtmlExporter {
    pub full_document: bool,
}

impl HtmlExporter {
    pub fn new() -> Self {
        Self { full_document: true }
    }
    pub fn fragment() -> Self {
        Self { full_document: false }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HtmlError {
    #[error("write error: {0}")]
    Write(#[from] std::fmt::Error),
}

impl Exporter for HtmlExporter {
    type Output = String;
    type Error = HtmlError;
    fn export(&self, doc: &Document, theme: &Theme) -> Result<String, HtmlError> {
        // Run the typed-tree cook pass first — fill/source desugar to
        // cells, same-address cells merge, col/row/format cascade. The
        // exporter then walks a normalized tree.
        let cooked = stem_parser::cook_document(doc);

        let mut out = String::new();
        if self.full_document {
            writeln!(out, "<!doctype html>")?;
            writeln!(
                out,
                "<html lang=\"{}\">",
                html_attr(cooked.metadata.get_str("locale").unwrap_or("en"))
            )?;
            writeln!(out, "<head>")?;
            writeln!(out, "<meta charset=\"utf-8\">")?;
            if let Some(t) = cooked.metadata.get_str("title") {
                writeln!(out, "<title>{}</title>", html_text(t))?;
            }
            writeln!(out, "<style>{}</style>", base_css(theme))?;
            writeln!(out, "</head>")?;
            writeln!(out, "<body>")?;
        }
        writeln!(out, "<div class=\"stem-doc\">")?;
        for block in &cooked.blocks {
            render_block(&mut out, block, theme)?;
        }
        writeln!(out, "</div>")?;
        if self.full_document {
            writeln!(out, "</body>")?;
            writeln!(out, "</html>")?;
        }
        Ok(out)
    }
}

pub(crate) fn render_block(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    // Per-element dispatch: each block element owns its own render fn
    // in `elements::<name>`. Elements not in the table fall through to
    // the generic block wrapper.
    if let Some(el) = elements::lookup_block(&b.name) {
        return (el.render)(out, b, theme);
    }
    render_fallback_block(out, b, theme)
}


/// Local copy of address parser — duplicated from cook to avoid a
/// cross-crate dependency on internals. Returns (col_idx, row_idx)
/// 0-based.
pub(crate) fn parse_cell_address(s: &str) -> Option<(u32, u32)> {
    if s.is_empty() {
        return None;
    }
    let split = s.find(|c: char| c.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let (col, row) = s.split_at(split);
    let mut n: u32 = 0;
    for c in col.chars() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        n = n.checked_mul(26)?.checked_add(c.to_ascii_uppercase() as u32 - b'A' as u32 + 1)?;
    }
    if n == 0 {
        return None;
    }
    let row_n: u32 = row.parse().ok()?;
    if row_n == 0 {
        return None;
    }
    Some((n - 1, row_n - 1))
}

pub(crate) fn format_col_letter(mut n: u32) -> String {
    let mut s = String::new();
    n += 1;
    while n > 0 {
        let r = (n - 1) % 26;
        s.insert(0, (b'A' + r as u8) as char);
        n = (n - 1) / 26;
    }
    s
}

fn render_fallback_block(
    out: &mut String,
    b: &Block,
    theme: &Theme,
) -> Result<(), std::fmt::Error> {
    writeln!(out, "<div data-stem=\"{}\">", html_attr(&b.name))?;
    match &b.body {
        Body::None => {}
        Body::Text(_) => render_text_body_inline(out, b, theme)?,
        Body::Children(_) => render_children_of(out, b, theme)?,
    }
    writeln!(out, "</div>")?;
    Ok(())
}

// -----------------------------------------------------------
// Helpers
// -----------------------------------------------------------

pub(crate) fn render_children_of(
    out: &mut String,
    b: &Block,
    theme: &Theme,
) -> Result<(), std::fmt::Error> {
    if let Body::Children(kids) = &b.body {
        for k in kids {
            render_block(out, k, theme)?;
        }
    }
    Ok(())
}

pub(crate) fn render_text_body_inline(
    out: &mut String,
    b: &Block,
    theme: &Theme,
) -> Result<(), std::fmt::Error> {
    if let Body::Text(pieces) = &b.body {
        for p in pieces {
            match p {
                TextPiece::Literal { text, .. } => write!(out, "{}", html_text(text))?,
                TextPiece::Inline(inline) => render_inline(out, inline, theme)?,
            }
        }
    }
    Ok(())
}

fn render_inline(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    // Per-element dispatch: each inline owns its own render fn in
    // `elements::<name>`. Unknown inlines fall through to the generic
    // tagged-span wrapper, preserving previous behavior.
    if let Some(el) = elements::lookup_inline(&b.name) {
        return (el.render)(out, b, theme);
    }
    let mut text = String::new();
    for s in b.body_text_pieces() {
        text.push_str(&s);
    }
    write!(
        out,
        "<span data-stem=\"{}\">{}</span>",
        html_attr(&b.name),
        html_text(&text)
    )?;
    Ok(())
}

pub(crate) trait BodyTextPieces {
    fn body_text_pieces(&self) -> Vec<String>;
}

impl BodyTextPieces for Block {
    fn body_text_pieces(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Body::Text(pieces) = &self.body {
            for p in pieces {
                if let TextPiece::Literal { text, .. } = p {
                    out.push(text.clone());
                }
            }
        }
        out
    }
}

pub(crate) fn grid_template_for(kind: &str) -> &'static str {
    match kind {
        "two-column" => "grid-template-columns:1fr 1fr;",
        "three-column" => "grid-template-columns:1fr 1fr 1fr;",
        "sidebar" => "grid-template-columns:1fr 2fr;",
        _ => "grid-template-columns:1fr;",
    }
}

fn base_css(theme: &Theme) -> String {
    let text = theme
        .resolve_color("text")
        .map(|c| c.to_hex())
        .unwrap_or_else(|| "#141820".into());
    let bg = theme
        .resolve_color("background")
        .map(|c| c.to_hex())
        .unwrap_or_else(|| "#ffffff".into());
    let rule = theme
        .resolve_color("rule")
        .map(|c| c.to_hex())
        .unwrap_or_else(|| "#d0d7de".into());
    format!(
        "body{{font-family:{body};color:{text};background:{bg};max-width:42rem;margin:2rem auto;padding:0 1rem;line-height:1.55;}}\
         h1,h2,h3,h4,h5,h6{{font-family:{heading};}}\
         table{{width:100%;}}\
         th,td{{border-color:{rule};}}\
         code{{font-family:{mono};background:#f6f8fa;padding:0 0.25em;border-radius:3px;}}\
         figure{{margin:1rem 0;}}\
         figcaption{{font-size:0.9em;color:#666;text-align:center;}}",
        body = theme.fonts.body,
        heading = theme.fonts.heading,
        mono = theme.fonts.mono,
        text = text,
        bg = bg,
        rule = rule,
    )
}

pub(crate) fn html_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

pub(crate) fn html_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}
