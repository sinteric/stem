//! HTML renderer — fully implemented. Produces a self-contained HTML
//! document with inline styles drawn from the theme.

use std::fmt::Write;

use stem_core::ast::*;
use stem_core::theme::Theme;

use crate::{intent, Renderer};

#[derive(Default)]
pub struct HtmlRenderer {
    /// If true, wrap the body in a full `<!doctype html>` document.
    /// Otherwise emit just a `<div class="stem-doc">` fragment.
    pub full_document: bool,
}

impl HtmlRenderer {
    pub fn new() -> Self {
        Self {
            full_document: true,
        }
    }

    pub fn fragment() -> Self {
        Self {
            full_document: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HtmlError {
    #[error("write error: {0}")]
    Write(#[from] std::fmt::Error),
}

impl Renderer for HtmlRenderer {
    type Output = String;
    type Error = HtmlError;

    fn render(&self, doc: &Document, theme: &Theme) -> Result<String, Self::Error> {
        let mut out = String::new();
        if self.full_document {
            writeln!(out, "<!doctype html>")?;
            writeln!(out, "<html lang=\"{}\">", html_attr(doc.metadata.get_str("locale").unwrap_or("en")))?;
            writeln!(out, "<head>")?;
            writeln!(out, "<meta charset=\"utf-8\">")?;
            if let Some(t) = doc.metadata.get_str("title") {
                writeln!(out, "<title>{}</title>", html_text(t))?;
            }
            writeln!(out, "<style>{}</style>", base_css(theme))?;
            writeln!(out, "</head>")?;
            writeln!(out, "<body>")?;
        }
        writeln!(out, "<div class=\"stem-doc\">")?;
        let cooked = stem_parser::cook_document(doc);
        for block in &cooked.blocks {
            render_block(&mut out, block, theme, 0)?;
        }
        writeln!(out, "</div>")?;
        if self.full_document {
            writeln!(out, "</body>")?;
            writeln!(out, "</html>")?;
        }
        Ok(out)
    }
}

fn render_block(
    out: &mut String,
    block: &Block,
    theme: &Theme,
    depth: usize,
) -> Result<(), std::fmt::Error> {
    match block {
        Block::Heading { level, runs, .. } => {
            write!(out, "<h{}>", level)?;
            for r in runs {
                render_inline(out, r, theme)?;
            }
            writeln!(out, "</h{}>", level)?;
        }
        Block::Paragraph(p) => {
            write!(out, "<p>")?;
            for r in &p.runs {
                render_inline(out, r, theme)?;
            }
            writeln!(out, "</p>")?;
        }
        Block::List { kind, items, .. } => {
            let tag = match kind {
                ListKind::Unordered => "ul",
                ListKind::Ordered => "ol",
            };
            writeln!(out, "<{}>", tag)?;
            for item in items {
                write!(out, "<li>")?;
                for r in &item.runs {
                    render_inline(out, r, theme)?;
                }
                writeln!(out, "</li>")?;
            }
            writeln!(out, "</{}>", tag)?;
        }
        Block::Call(c) => render_block_call(out, c, theme, depth)?,
    }
    Ok(())
}

fn render_block_call(
    out: &mut String,
    c: &FunctionCall,
    theme: &Theme,
    depth: usize,
) -> Result<(), std::fmt::Error> {
    if intent::is_section(c) {
        // `section(id)` is a marker section (no body); `section(id)(body)`
        // has both. The renderer treats a single-arg call as id-only so
        // marker sections like `section(toc)` don't render their id as a
        // paragraph.
        let (id, has_body) = match c.args.len() {
            0 => (None, false),
            1 => (c.args.first().and_then(|a| content_as_text(a)), false),
            _ => (c.header().and_then(content_as_text), true),
        };
        if let Some(ref id) = id {
            writeln!(out, "<section data-id=\"{}\">", html_attr(id))?;
        } else {
            writeln!(out, "<section>")?;
        }
        if has_body {
            render_call_body(out, c, theme, depth + 1)?;
        } else if id.as_deref() == Some("toc") {
            writeln!(
                out,
                "<nav class=\"stem-toc\" aria-label=\"Table of contents\"><!-- generated --></nav>"
            )?;
        }
        writeln!(out, "</section>")?;
        return Ok(());
    }
    if intent::is_layout(c) {
        let kind = c.header().and_then(content_as_text).unwrap_or_default();
        writeln!(
            out,
            "<div class=\"stem-layout\" data-layout=\"{}\" style=\"display:grid;gap:1rem;{}\">",
            html_attr(&kind),
            grid_template_for(&kind),
        )?;
        render_call_body(out, c, theme, depth + 1)?;
        writeln!(out, "</div>")?;
        return Ok(());
    }
    if intent::is_col(c) {
        writeln!(out, "<div class=\"stem-col\">")?;
        render_call_body(out, c, theme, depth + 1)?;
        writeln!(out, "</div>")?;
        return Ok(());
    }
    if intent::is_table(c) {
        let border = c
            .properties
            .iter()
            .find(|p| p.key == "border")
            .map(|p| p.value.as_str())
            .unwrap_or("none");
        let style = match border {
            "outer" => "border:1px solid currentColor;border-collapse:collapse;",
            "all" => "border:1px solid currentColor;border-collapse:collapse;",
            _ => "border-collapse:collapse;",
        };
        writeln!(out, "<table data-border=\"{}\" style=\"{}\">", border, style)?;
        for child in c.body() {
            if let Content::Call(row) = child {
                if intent::is_row(row) {
                    render_row(out, row, border, theme)?;
                }
            }
        }
        writeln!(out, "</table>")?;
        return Ok(());
    }
    if intent::is_pagebreak(c) {
        writeln!(
            out,
            "<div class=\"stem-pagebreak\" style=\"page-break-after:always;\"></div>"
        )?;
        return Ok(());
    }
    if intent::is_toc(c) {
        writeln!(
            out,
            "<nav class=\"stem-toc\" aria-label=\"Table of contents\"><!-- toc generated at render time --></nav>"
        )?;
        return Ok(());
    }
    if intent::is_note(c) {
        let text = call_body_text(c);
        writeln!(
            out,
            "<aside class=\"stem-note\" style=\"color:{}; font-size: 0.9em;\">{}</aside>",
            theme
                .resolve_color("muted")
                .map(|c| c.to_hex())
                .unwrap_or_else(|| "#666".into()),
            html_text(&text)
        )?;
        return Ok(());
    }
    // Fallback for unknown block calls — render body as a generic <div>
    // with the name in a data attribute. Renderers should never panic on
    // unknown calls.
    writeln!(out, "<div data-stem=\"{}\">", html_attr(&c.name))?;
    render_call_body(out, c, theme, depth + 1)?;
    writeln!(out, "</div>")?;
    Ok(())
}

fn render_call_body(
    out: &mut String,
    c: &FunctionCall,
    theme: &Theme,
    depth: usize,
) -> Result<(), std::fmt::Error> {
    let cooked = stem_parser::cook_run(c.body());
    for b in &cooked {
        render_block(out, b, theme, depth)?;
    }
    Ok(())
}

fn render_row(
    out: &mut String,
    row: &FunctionCall,
    border: &str,
    theme: &Theme,
) -> Result<(), std::fmt::Error> {
    let is_header = row
        .header()
        .and_then(content_as_text)
        .map(|s| s.trim() == "header")
        .unwrap_or(false);
    writeln!(out, "<tr>")?;
    for child in row.body() {
        if let Content::Call(cell) = child {
            if intent::is_cell(cell) {
                render_cell(out, cell, is_header, border, theme)?;
            }
        }
    }
    writeln!(out, "</tr>")?;
    Ok(())
}

fn render_cell(
    out: &mut String,
    cell: &FunctionCall,
    is_header: bool,
    border: &str,
    theme: &Theme,
) -> Result<(), std::fmt::Error> {
    let tag = if is_header { "th" } else { "td" };
    let mut attrs = String::new();
    let mut style = String::new();
    if border == "all" || border == "outer" {
        style.push_str("border:1px solid currentColor;");
    }
    style.push_str("padding:0.25rem 0.5rem;");
    for p in &cell.properties {
        match p.key.as_str() {
            "span" => write!(attrs, " colspan=\"{}\"", html_attr(p.value.as_str())).unwrap(),
            "rowspan" => write!(attrs, " rowspan=\"{}\"", html_attr(p.value.as_str())).unwrap(),
            "align" => write!(style, "text-align:{};", html_attr(p.value.as_str())).unwrap(),
            "bg" => {
                if let Some(c) = theme.resolve_color(p.value.as_str()) {
                    write!(style, "background:{};", c.to_hex()).unwrap();
                }
            }
            _ => {}
        }
    }
    write!(out, "<{}{} style=\"{}\">", tag, attrs, style)?;
    // cell body: render as inline runs (cells rarely have multi-paragraph content)
    let cooked = stem_parser::cook_run(cell.body());
    for b in &cooked {
        // For cells, paragraphs are flattened to inline.
        if let Block::Paragraph(p) = b {
            for r in &p.runs {
                render_inline(out, r, theme)?;
            }
        } else {
            render_block(out, b, theme, 0)?;
        }
    }
    writeln!(out, "</{}>", tag)?;
    Ok(())
}

fn render_inline(out: &mut String, inline: &Inline, theme: &Theme) -> Result<(), std::fmt::Error> {
    match inline {
        Inline::Text { text, style, .. } => {
            let escaped = html_text(text);
            if style.bold && style.italic {
                write!(out, "<strong><em>{}</em></strong>", escaped)?;
            } else if style.bold {
                write!(out, "<strong>{}</strong>", escaped)?;
            } else if style.italic {
                write!(out, "<em>{}</em>", escaped)?;
            } else if style.code {
                write!(out, "<code>{}</code>", escaped)?;
            } else {
                write!(out, "{}", escaped)?;
            }
        }
        Inline::Call(c) => render_inline_call(out, c, theme)?,
    }
    Ok(())
}

fn render_inline_call(
    out: &mut String,
    c: &FunctionCall,
    theme: &Theme,
) -> Result<(), std::fmt::Error> {
    if intent::is_text_span(c) {
        let mut style = String::new();
        for p in &c.properties {
            match p.key.as_str() {
                "color" => {
                    if let Some(col) = theme.resolve_color(p.value.as_str()) {
                        write!(style, "color:{};", col.to_hex()).unwrap();
                    }
                }
                "bg" => {
                    if let Some(col) = theme.resolve_color(p.value.as_str()) {
                        write!(style, "background:{};", col.to_hex()).unwrap();
                    }
                }
                "weight" => match p.value.as_str() {
                    "light" => style.push_str("font-weight:300;"),
                    "regular" => style.push_str("font-weight:400;"),
                    "bold" => style.push_str("font-weight:700;"),
                    _ => {}
                },
                _ => {}
            }
        }
        write!(out, "<span style=\"{}\">", style)?;
        let text = call_body_text(c);
        write!(out, "{}", html_text(&text))?;
        write!(out, "</span>")?;
        return Ok(());
    }
    if intent::is_footnote(c) {
        let text = call_body_text(c);
        write!(
            out,
            "<sup class=\"stem-footnote\" title=\"{}\">*</sup>",
            html_attr(&text)
        )?;
        return Ok(());
    }
    if intent::is_date(c) {
        let text = call_body_text(c);
        write!(out, "<time>{}</time>", html_text(&text))?;
        return Ok(());
    }
    // Fallback: render the body text inside a span tagged with the
    // function name.
    let text = call_body_text(c);
    write!(
        out,
        "<span data-stem=\"{}\">{}</span>",
        html_attr(&c.name),
        html_text(&text)
    )?;
    Ok(())
}

fn content_as_text(content: &[Content]) -> Option<String> {
    let mut out = String::new();
    for c in content {
        match c {
            Content::Text(t) => out.push_str(t.text.trim()),
            Content::Call(_) => return None, // not a plain text arg
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn call_body_text(c: &FunctionCall) -> String {
    let mut out = String::new();
    for item in c.body() {
        if let Content::Text(t) = item {
            out.push_str(&t.text);
        }
    }
    out
}

fn grid_template_for(kind: &str) -> &'static str {
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
         h1,h2,h3,h4{{font-family:{heading};}}\
         table{{width:100%;}}\
         th,td{{border-color:{rule};}}\
         .stem-pagebreak{{height:0;border-top:1px dashed {rule};margin:2rem 0;}}\
         code{{font-family:{mono};background:#f6f8fa;padding:0 0.25em;border-radius:3px;}}\
        ",
        body = theme.fonts.body,
        heading = theme.fonts.heading,
        mono = theme.fonts.mono,
        text = text,
        bg = bg,
        rule = rule,
    )
}

fn html_text(s: &str) -> String {
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

fn html_attr(s: &str) -> String {
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
