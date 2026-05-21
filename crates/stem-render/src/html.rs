//! HTML renderer.
//!
//! Walks a `stem_core::ast::Document` and produces HTML. Doesn't
//! consult any registry — renders by element name, with a generic
//! fallback for unknown blocks ().

use std::fmt::Write;

use stem_core::ast::*;
use stem_core::theme::Theme;

use crate::Renderer;

#[derive(Default)]
pub struct HtmlRenderer {
    pub full_document: bool,
}

impl HtmlRenderer {
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

impl HtmlRenderer {
    pub fn render(&self, doc: &Document, theme: &Theme) -> Result<String, HtmlError> {
        // Run the typed-tree cook pass first — fill/source desugar to
        // cells, same-address cells merge, col/row/format cascade. The
        // renderer then walks a normalized tree.
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

impl Renderer for HtmlRenderer {
    type Output = String;
    type Error = HtmlError;
    fn render(
        &self,
        _doc: &stem_core::ast::Document,
        _theme: &Theme,
    ) -> Result<String, Self::Error> {
        Err(HtmlError::Write(std::fmt::Error))
    }
}

fn render_block(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    match b.name.as_str() {
        "section" => render_section(out, b, theme),
        "layout" => render_layout(out, b, theme),
        "col" => render_col(out, b, theme),
        "pagebreak" => writeln!(
            out,
            "<div class=\"stem-pagebreak\" style=\"page-break-after:always;\"></div>"
        ),
        "hr" => writeln!(out, "<hr>"),
        "h1" => render_heading(out, b, 1),
        "h2" => render_heading(out, b, 2),
        "h3" => render_heading(out, b, 3),
        "h4" => render_heading(out, b, 4),
        "h5" => render_heading(out, b, 5),
        "h6" => render_heading(out, b, 6),
        "p" => render_paragraph(out, b, theme),
        "note" => render_note(out, b, theme),
        "blockquote" => render_blockquote(out, b, theme),
        "image" => render_image(out, b),
        "ol" => render_list(out, b, theme, true),
        "ul" => render_list(out, b, theme, false),
        "li" => render_list_item(out, b, theme),
        "table" => render_table(out, b, theme),
        "row" => render_row(out, b, theme, false),
        "cell" => render_cell(out, b, theme, false),
        "date" => render_date_block(out, b),
        "code" => render_code_block(out, b),
        "slide" => render_slide(out, b, theme),
        "title" => render_slide_title(out, b, theme),
        "bullets" => render_list(out, b, theme, false), // bullets render like ul
        "item" => render_list_item(out, b, theme),
        "speaker-note" => render_speaker_note(out, b),
        "sheet" => render_sheet(out, b, theme),
        _ => render_fallback_block(out, b, theme),
    }
}

fn render_section(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let id = b.prop_str("id");
    match id {
        Some(id) => writeln!(out, "<section data-id=\"{}\">", html_attr(id))?,
        None => writeln!(out, "<section>")?,
    }
    if id == Some("toc") && b.body.is_none() {
        writeln!(
            out,
            "<nav class=\"stem-toc\" aria-label=\"Table of contents\"><!-- generated --></nav>"
        )?;
    } else {
        render_children_of(out, b, theme)?;
    }
    writeln!(out, "</section>")?;
    Ok(())
}

fn render_layout(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let kind = b.prop_str("kind").unwrap_or("two-column");
    writeln!(
        out,
        "<div class=\"stem-layout\" data-layout=\"{}\" style=\"display:grid;gap:1rem;{}\">",
        html_attr(kind),
        grid_template_for(kind),
    )?;
    render_children_of(out, b, theme)?;
    writeln!(out, "</div>")?;
    Ok(())
}

fn render_col(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    writeln!(out, "<div class=\"stem-col\">")?;
    render_children_of(out, b, theme)?;
    writeln!(out, "</div>")?;
    Ok(())
}

fn render_heading(out: &mut String, b: &Block, level: u8) -> Result<(), std::fmt::Error> {
    write!(out, "<h{}", level)?;
    if let Some(id) = b.prop_str("id") {
        write!(out, " id=\"{}\"", html_attr(id))?;
    }
    write!(out, ">")?;
    render_text_body_inline(out, b)?;
    writeln!(out, "</h{}>", level)?;
    Ok(())
}

fn render_paragraph(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    write!(out, "<p")?;
    if let Some(a) = b.prop_str("align") {
        write!(out, " style=\"text-align:{};\"", html_attr(a))?;
    }
    write!(out, ">")?;
    render_text_body_inline(out, b)?;
    writeln!(out, "</p>")?;
    Ok(())
}

fn render_note(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let bg = theme
        .resolve_color("gray")
        .map(|c| c.to_hex())
        .unwrap_or_else(|| "#f6f8fa".into());
    let kind = b.prop_str("kind").unwrap_or("info");
    writeln!(
        out,
        "<aside class=\"stem-note stem-note-{}\" style=\"display:block;padding:0.5rem 0.75rem;\
         background:{};border-left:3px solid #8b949e;margin:1rem 0;\">",
        html_attr(kind),
        bg
    )?;
    render_text_body_inline(out, b)?;
    writeln!(out, "</aside>")?;
    Ok(())
}

fn render_blockquote(
    out: &mut String,
    b: &Block,
    _theme: &Theme,
) -> Result<(), std::fmt::Error> {
    write!(out, "<blockquote")?;
    if let Some(c) = b.prop_str("cite") {
        write!(out, " cite=\"{}\"", html_attr(c))?;
    }
    write!(out, ">")?;
    render_text_body_inline(out, b)?;
    writeln!(out, "</blockquote>")?;
    Ok(())
}

fn render_image(out: &mut String, b: &Block) -> Result<(), std::fmt::Error> {
    let src = b.prop_str("src").unwrap_or("");
    let alt = b.prop_str("alt").unwrap_or("");
    write!(
        out,
        "<figure><img src=\"{}\" alt=\"{}\"",
        html_attr(src),
        html_attr(alt)
    )?;
    if let Some(w) = b.prop_str("w") {
        write!(out, " style=\"width:{};\"", html_attr(w))?;
    }
    writeln!(out, ">")?;
    if let Some(c) = b.prop_str("caption") {
        writeln!(out, "<figcaption>{}</figcaption>", html_text(c))?;
    }
    writeln!(out, "</figure>")?;
    Ok(())
}

fn render_list(
    out: &mut String,
    b: &Block,
    theme: &Theme,
    ordered: bool,
) -> Result<(), std::fmt::Error> {
    let tag = if ordered { "ol" } else { "ul" };
    write!(out, "<{}", tag)?;
    if let Some(start) = b.prop_str("start") {
        write!(out, " start=\"{}\"", html_attr(start))?;
    }
    if let Some(style) = b.prop_str("style") {
        // Render style as a data attr; CSS list-style-type mapping is renderer-specific.
        write!(out, " data-style=\"{}\"", html_attr(style))?;
    }
    writeln!(out, ">")?;
    render_children_of(out, b, theme)?;
    writeln!(out, "</{}>", tag)?;
    Ok(())
}

fn render_list_item(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    write!(out, "<li")?;
    if let Some(at) = b.prop_str("at") {
        write!(out, " value=\"{}\"", html_attr(at))?;
    }
    write!(out, ">")?;
    match &b.body {
        Body::Text(_) => render_text_body_inline(out, b)?,
        Body::Children(_) => render_children_of(out, b, theme)?,
        Body::None => {}
    }
    writeln!(out, "</li>")?;
    Ok(())
}

fn render_table(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let border = b.prop_str("border").unwrap_or("none");
    let style = match border {
        "outer" | "all" => "border:1px solid currentColor;border-collapse:collapse;",
        _ => "border-collapse:collapse;",
    };
    writeln!(out, "<table data-border=\"{}\" style=\"{}\">", border, style)?;
    if let Some(c) = b.prop_str("caption") {
        writeln!(out, "<caption>{}</caption>", html_text(c))?;
    }
    if let Body::Children(children) = &b.body {
        for child in children {
            if child.name == "row" {
                let is_header = child.prop_str("kind") == Some("header");
                render_row(out, child, theme, is_header)?;
            }
        }
    }
    writeln!(out, "</table>")?;
    Ok(())
}

fn render_row(
    out: &mut String,
    b: &Block,
    theme: &Theme,
    is_header: bool,
) -> Result<(), std::fmt::Error> {
    writeln!(out, "<tr>")?;
    if let Body::Children(children) = &b.body {
        for child in children {
            if child.name == "cell" {
                render_cell(out, child, theme, is_header)?;
            }
        }
    }
    writeln!(out, "</tr>")?;
    Ok(())
}

fn render_cell(
    out: &mut String,
    b: &Block,
    theme: &Theme,
    is_header: bool,
) -> Result<(), std::fmt::Error> {
    let tag = if is_header { "th" } else { "td" };
    let mut style = String::from("padding:0.25rem 0.5rem;border:1px solid currentColor;");
    let mut attrs = String::new();
    for p in &b.properties {
        match p.key.as_str() {
            "colspan" => write!(attrs, " colspan=\"{}\"", html_attr(p.value.as_str())).unwrap(),
            "rowspan" => write!(attrs, " rowspan=\"{}\"", html_attr(p.value.as_str())).unwrap(),
            "align" => write!(style, "text-align:{};", html_attr(p.value.as_str())).unwrap(),
            "valign" => write!(style, "vertical-align:{};", html_attr(p.value.as_str())).unwrap(),
            "bg" => {
                if let Some(c) = theme.resolve_color(p.value.as_str()) {
                    write!(style, "background:{};", c.to_hex()).unwrap();
                }
            }
            _ => {}
        }
    }
    write!(out, "<{}{} style=\"{}\">", tag, attrs, style)?;
    render_text_body_inline(out, b)?;
    writeln!(out, "</{}>", tag)?;
    Ok(())
}

fn render_date_block(out: &mut String, b: &Block) -> Result<(), std::fmt::Error> {
    write!(out, "<time>")?;
    render_text_body_inline(out, b)?;
    writeln!(out, "</time>")?;
    Ok(())
}

fn render_code_block(out: &mut String, b: &Block) -> Result<(), std::fmt::Error> {
    let lang = b.prop_str("lang").unwrap_or("");
    write!(
        out,
        "<pre><code class=\"language-{}\">",
        html_attr(lang)
    )?;
    if let Body::Text(pieces) = &b.body {
        for p in pieces {
            if let TextPiece::Literal { text, .. } = p {
                write!(out, "{}", html_text(text))?;
            }
        }
    }
    writeln!(out, "</code></pre>")?;
    Ok(())
}

fn render_slide(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let mut style = String::from(
        "page-break-after:always;min-height:5in;padding:1rem;border:1px dashed #aaa;\
         margin-bottom:1rem;",
    );
    if let Some(bg) = b.prop_str("background") {
        if let Some(c) = theme.resolve_color(bg) {
            write!(style, "background:{};", c.to_hex()).unwrap();
        }
    }
    let id = b.prop_str("id").unwrap_or("");
    let layout = b.prop_str("layout").unwrap_or("");
    writeln!(
        out,
        "<section class=\"stem-slide\" data-id=\"{}\" data-layout=\"{}\" style=\"{}\">",
        html_attr(id),
        html_attr(layout),
        style,
    )?;
    render_children_of(out, b, theme)?;
    writeln!(out, "</section>")?;
    Ok(())
}

fn render_slide_title(
    out: &mut String,
    b: &Block,
    _theme: &Theme,
) -> Result<(), std::fmt::Error> {
    write!(out, "<h1 class=\"stem-slide-title\">")?;
    render_text_body_inline(out, b)?;
    writeln!(out, "</h1>")?;
    Ok(())
}

fn render_speaker_note(out: &mut String, b: &Block) -> Result<(), std::fmt::Error> {
    write!(
        out,
        "<aside class=\"stem-speaker-note\" hidden style=\"display:none;\">"
    )?;
    if let Body::Text(pieces) = &b.body {
        for p in pieces {
            if let TextPiece::Literal { text, .. } = p {
                write!(out, "{}", html_text(text))?;
            }
        }
    }
    writeln!(out, "</aside>")?;
    Ok(())
}

fn render_sheet(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let name = b.prop_str("name").unwrap_or_else(|| b.prop_str("id").unwrap_or(""));
    writeln!(
        out,
        "<section class=\"stem-sheet\" data-id=\"{}\">",
        html_attr(b.prop_str("id").unwrap_or(""))
    )?;
    if !name.is_empty() {
        writeln!(out, "<h3 class=\"stem-sheet-name\">{}</h3>", html_text(name))?;
    }

    // Index cells by (col_idx, row_idx) using the cooked address.
    // After cook, every `cell[at:X]` should have its cascaded props
    // already applied.
    let mut by_pos: std::collections::HashMap<(u32, u32), &Block> = std::collections::HashMap::new();
    let mut max_col: i64 = -1;
    let mut max_row: i64 = -1;

    // Build a source map for the formula evaluator. Cells whose body
    // is a single `@formula(...)` inline element become CellSource::Formula
    // — extracting the formula's body text. Everything else (plain text
    // bodies, numbers) becomes CellSource::Literal.
    let mut cell_sources: std::collections::HashMap<(u32, u32), crate::formula::CellSource> =
        std::collections::HashMap::new();

    if let Body::Children(kids) = &b.body {
        for child in kids {
            if child.name != "cell" {
                continue;
            }
            let at = match child.prop_str("at") {
                Some(s) => s,
                None => continue,
            };
            if let Some((c, r)) = parse_cell_address(at) {
                by_pos.insert((c, r), child);
                if let Some(source) = extract_cell_source(child) {
                    cell_sources.insert((c, r), source);
                }
                if (c as i64) > max_col {
                    max_col = c as i64;
                }
                if (r as i64) > max_row {
                    max_row = r as i64;
                }
            }
        }
    }

    // Evaluate all formulas. The returned map has Num/Str/Error per cell.
    let evaluated: std::collections::HashMap<(u32, u32), crate::formula::Value> =
        crate::formula::evaluate_sheet(&cell_sources);

    if max_col < 0 || max_row < 0 {
        writeln!(
            out,
            "<p class=\"stem-sheet-empty\" style=\"color:#888;font-style:italic;\">(empty sheet)</p>"
        )?;
    } else {
        let max_col = max_col as u32;
        let max_row = max_row as u32;
        writeln!(
            out,
            "<table class=\"stem-sheet-grid\" style=\"border-collapse:collapse;font-family:ui-monospace,monospace;\">"
        )?;

        // Column header row (A, B, C, ...).
        writeln!(out, "<thead><tr>")?;
        writeln!(
            out,
            "<th style=\"background:#f6f8fa;border:1px solid #d0d7de;padding:0.15rem 0.4rem;color:#888;\"></th>"
        )?;
        for c in 0..=max_col {
            writeln!(
                out,
                "<th style=\"background:#f6f8fa;border:1px solid #d0d7de;padding:0.15rem 0.4rem;color:#888;\">{}</th>",
                format_col_letter(c)
            )?;
        }
        writeln!(out, "</tr></thead>")?;

        writeln!(out, "<tbody>")?;
        for r in 0..=max_row {
            writeln!(out, "<tr>")?;
            // Row header
            writeln!(
                out,
                "<th style=\"background:#f6f8fa;border:1px solid #d0d7de;padding:0.15rem 0.4rem;color:#888;\">{}</th>",
                r + 1
            )?;
            for c in 0..=max_col {
                match by_pos.get(&(c, r)) {
                    Some(cell) => {
                        let value = evaluated.get(&(c, r));
                        render_sheet_cell(out, cell, theme, value)?;
                    }
                    None => writeln!(
                        out,
                        "<td style=\"border:1px solid #d0d7de;padding:0.15rem 0.4rem;\"></td>"
                    )?,
                }
            }
            writeln!(out, "</tr>")?;
        }
        writeln!(out, "</tbody>")?;
        writeln!(out, "</table>")?;
    }

    writeln!(out, "</section>")?;
    Ok(())
}

fn render_sheet_cell(
    out: &mut String,
    cell: &Block,
    theme: &Theme,
    evaluated: Option<&crate::formula::Value>,
) -> Result<(), std::fmt::Error> {
    let mut style = String::from("border:1px solid #d0d7de;padding:0.15rem 0.4rem;");
    for p in &cell.properties {
        match p.key.as_str() {
            "bg" => {
                if let Some(c) = theme.resolve_color(p.value.as_str()) {
                    write!(style, "background:{};", c.to_hex()).unwrap();
                }
            }
            "weight" => match p.value.as_str() {
                "light" => style.push_str("font-weight:300;"),
                "regular" => style.push_str("font-weight:400;"),
                "bold" => style.push_str("font-weight:700;"),
                _ => {}
            },
            "align" => write!(style, "text-align:{};", html_attr(p.value.as_str())).unwrap(),
            "valign" => write!(style, "vertical-align:{};", html_attr(p.value.as_str())).unwrap(),
            _ => {}
        }
    }
    let fmt = cell.prop_str("fmt");
    let raw_body = cell.plain_text().unwrap_or_default();

    // Detect "this cell is a formula cell" by looking for the @formula
    // inline element in its body. Mirrors extract_cell_source.
    let formula_inline = if let Body::Text(pieces) = &cell.body {
        pieces.iter().find_map(|p| match p {
            TextPiece::Inline(b) if b.name == "formula" => Some(b),
            _ => None,
        })
    } else {
        None
    };
    let is_formula = formula_inline.is_some();

    // Display: evaluated value formatted per fmt if available, otherwise raw body.
    let display = if let Some(value) = evaluated {
        if is_formula || matches!(value, crate::formula::Value::Num(_)) {
            crate::formula::format_value(value, fmt)
        } else {
            raw_body.clone()
        }
    } else {
        raw_body.clone()
    };

    // Title attr shows the original formula text on hover.
    let title = match formula_inline {
        Some(b) => format!(
            " title=\"@formula({})\"",
            html_attr(&b.plain_text().unwrap_or_default())
        ),
        None => String::new(),
    };

    write!(
        out,
        "<td style=\"{}\" data-fmt=\"{}\"{}>",
        style,
        html_attr(fmt.unwrap_or("")),
        title
    )?;
    if is_formula {
        write!(out, "<span class=\"stem-formula-value\">{}</span>", html_text(&display))?;
    } else {
        write!(out, "{}", html_text(&display))?;
    }
    writeln!(out, "</td>")?;
    Ok(())
}

/// Inspect a `cell[at:X](body)` and decide whether its body is a
/// formula or a literal. Returns the corresponding `CellSource`, or
/// `None` if the cell has no body at all.
///
/// A formula cell has a body of exactly one `@formula(...)` inline
/// element (with optional surrounding whitespace). Mixed bodies (text
/// + `@formula` + text) are treated as literals — the inline `@formula`
/// still renders via the normal inline path when displaying the cell.
fn extract_cell_source(cell: &Block) -> Option<crate::formula::CellSource> {
    let pieces = match &cell.body {
        Body::Text(p) => p,
        _ => return None,
    };
    // Walk pieces, skip whitespace-only literals, find one inline @formula.
    let mut found: Option<&Block> = None;
    let mut had_other = false;
    for p in pieces {
        match p {
            TextPiece::Literal { text, .. } => {
                if !text.trim().is_empty() {
                    had_other = true;
                }
            }
            TextPiece::Inline(b) if b.name == "formula" => {
                if found.is_some() {
                    had_other = true; // multiple formulas — treat as literal
                }
                found = Some(b);
            }
            TextPiece::Inline(_) => {
                had_other = true;
            }
        }
    }
    if let (Some(inline), false) = (found, had_other) {
        let text = inline.plain_text().unwrap_or_default();
        return Some(crate::formula::CellSource::Formula(text));
    }
    // Literal body — concatenate text pieces' content (excluding inlines).
    let mut s = String::new();
    for p in pieces {
        if let TextPiece::Literal { text, .. } = p {
            s.push_str(text);
        }
    }
    if s.is_empty() && pieces.is_empty() {
        return None;
    }
    Some(crate::formula::CellSource::Literal(s))
}

/// Local copy of address parser — duplicated from cook to avoid a
/// cross-crate dependency on internals. Returns (col_idx, row_idx)
/// 0-based.
fn parse_cell_address(s: &str) -> Option<(u32, u32)> {
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

fn format_col_letter(mut n: u32) -> String {
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
        Body::Text(_) => render_text_body_inline(out, b)?,
        Body::Children(_) => render_children_of(out, b, theme)?,
    }
    writeln!(out, "</div>")?;
    Ok(())
}

// -----------------------------------------------------------
// Helpers
// -----------------------------------------------------------

fn render_children_of(
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

fn render_text_body_inline(out: &mut String, b: &Block) -> Result<(), std::fmt::Error> {
    if let Body::Text(pieces) = &b.body {
        for p in pieces {
            match p {
                TextPiece::Literal { text, .. } => write!(out, "{}", html_text(text))?,
                TextPiece::Inline(inline) => render_inline(out, inline)?,
            }
        }
    }
    Ok(())
}

fn render_inline(out: &mut String, b: &Block) -> Result<(), std::fmt::Error> {
    match b.name.as_str() {
        "text" => {
            let mut style = String::new();
            let theme = Theme::default();
            for p in &b.properties {
                match p.key.as_str() {
                    "color" => {
                        if let Some(c) = theme.resolve_color(p.value.as_str()) {
                            write!(style, "color:{};", c.to_hex()).unwrap();
                        }
                    }
                    "bg" => {
                        if let Some(c) = theme.resolve_color(p.value.as_str()) {
                            write!(style, "background:{};", c.to_hex()).unwrap();
                        }
                    }
                    "weight" => match p.value.as_str() {
                        "light" => style.push_str("font-weight:300;"),
                        "regular" => style.push_str("font-weight:400;"),
                        "bold" => style.push_str("font-weight:700;"),
                        _ => {}
                    },
                    "style" => match p.value.as_str() {
                        "italic" | "oblique" => style.push_str("font-style:italic;"),
                        "normal" => style.push_str("font-style:normal;"),
                        _ => {}
                    },
                    "decoration" => match p.value.as_str() {
                        "underline" => style.push_str("text-decoration:underline;"),
                        "strike" => style.push_str("text-decoration:line-through;"),
                        "none" => style.push_str("text-decoration:none;"),
                        _ => {}
                    },
                    _ => {}
                }
            }
            write!(out, "<span style=\"{}\">", style)?;
            for p in &b.body_text_pieces() {
                write!(out, "{}", html_text(p))?;
            }
            write!(out, "</span>")?;
        }
        "footnote" => {
            let mut text = String::new();
            for s in b.body_text_pieces() {
                text.push_str(&s);
            }
            write!(
                out,
                "<sup class=\"stem-footnote\" title=\"{}\">*</sup>",
                html_attr(&text)
            )?;
        }
        "code" => {
            let mut text = String::new();
            for s in b.body_text_pieces() {
                text.push_str(&s);
            }
            write!(out, "<code>{}</code>", html_text(&text))?;
        }
        "link" => {
            let to = b.prop_str("to").unwrap_or("#");
            write!(out, "<a href=\"{}\"", html_attr(to))?;
            if let Some(t) = b.prop_str("title") {
                write!(out, " title=\"{}\"", html_attr(t))?;
            }
            write!(out, ">")?;
            for s in b.body_text_pieces() {
                write!(out, "{}", html_text(&s))?;
            }
            write!(out, "</a>")?;
        }
        "date" => {
            let mut text = String::new();
            for s in b.body_text_pieces() {
                text.push_str(&s);
            }
            write!(out, "<time>{}</time>", html_text(&text))?;
        }
        "mention" => {
            let mut text = String::new();
            for s in b.body_text_pieces() {
                text.push_str(&s);
            }
            write!(
                out,
                "<span class=\"stem-mention\">{}</span>",
                html_text(&text)
            )?;
        }
        "math" => {
            let mut text = String::new();
            for s in b.body_text_pieces() {
                text.push_str(&s);
            }
            write!(out, "<span class=\"stem-math\">{}</span>", html_text(&text))?;
        }
        _ => {
            // Unknown inline → wrap in a tagged span
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
        }
    }
    Ok(())
}

trait BodyTextPieces {
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
