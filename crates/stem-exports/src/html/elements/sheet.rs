//! `sheet` — spreadsheet rendering.
//!
//! After the cook pass has applied col/row/format cascades to each
//! `cell[at:...]`, this renderer walks the cells, evaluates formulas,
//! and emits a 2D grid with column-letter and row-number headers.
//!
//! Internal helpers (`render_sheet_cell`, `extract_cell_source`) are
//! private to this module — they exist only to keep the main render
//! function readable.

use std::collections::HashMap;
use std::fmt::Write;

use stem_core::ast::{Block, Body, TextPiece};
use stem_core::theme::Theme;

use super::super::{format_col_letter, html_attr, html_text, parse_cell_address};
use super::HtmlElement;
use stem_types::formula;

pub const SHEET: HtmlElement = HtmlElement {
    name: "sheet",
    render,
};

fn render(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let name = b
        .prop_str("name")
        .unwrap_or_else(|| b.prop_str("id").unwrap_or(""));
    writeln!(
        out,
        "<section class=\"stem-sheet\" data-id=\"{}\">",
        html_attr(b.prop_str("id").unwrap_or(""))
    )?;
    if !name.is_empty() {
        writeln!(out, "<h3 class=\"stem-sheet-name\">{}</h3>", html_text(name))?;
    }

    let mut by_pos: HashMap<(u32, u32), &Block> = HashMap::new();
    let mut max_col: i64 = -1;
    let mut max_row: i64 = -1;
    let mut cell_sources: HashMap<(u32, u32), formula::CellSource> = HashMap::new();

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

    let evaluated: HashMap<(u32, u32), formula::Value> = formula::evaluate_sheet(&cell_sources);

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
    evaluated: Option<&formula::Value>,
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

    let formula_inline = if let Body::Text(pieces) = &cell.body {
        pieces.iter().find_map(|p| match p {
            TextPiece::Inline(b) if b.name == "formula" => Some(b),
            _ => None,
        })
    } else {
        None
    };
    let is_formula = formula_inline.is_some();

    let display = if let Some(value) = evaluated {
        if is_formula || matches!(value, formula::Value::Num(_)) {
            formula::format_value(value, fmt)
        } else {
            raw_body.clone()
        }
    } else if let (Some(fmt_kind), Ok(n)) = (fmt, raw_body.trim().parse::<f64>()) {
        // Literal numeric cell — apply the same formatter formula cells
        // use, so `cell[at:B2, fmt:currency](42000)` renders as $42,000.00.
        formula::format_value(&formula::Value::Num(n), Some(fmt_kind))
    } else {
        raw_body.clone()
    };

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
        write!(
            out,
            "<span class=\"stem-formula-value\">{}</span>",
            html_text(&display)
        )?;
    } else {
        write!(out, "{}", html_text(&display))?;
    }
    writeln!(out, "</td>")?;
    Ok(())
}

/// Inspect a `cell[at:X](body)` and decide whether its body is a
/// formula or a literal. Returns the corresponding `CellSource`, or
/// `None` if the cell has no body at all.
fn extract_cell_source(cell: &Block) -> Option<formula::CellSource> {
    let pieces = match &cell.body {
        Body::Text(p) => p,
        _ => return None,
    };
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
                    had_other = true;
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
        return Some(formula::CellSource::Formula(text));
    }
    let s = cell.plain_text().unwrap_or_default();
    if s.trim().is_empty() {
        return None;
    }
    Some(formula::CellSource::Literal(s))
}
