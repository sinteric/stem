//! `table` — document-table container.
//!
//! Mirrors the docx exporter's property surface:
//!   table[border:all|outer|none, stripe:true,
//!         indent:Npt, widths:"a,b,c,d",
//!         row-height:Npt, row-height-rule:atLeast|exact|auto,
//!         caption:"…"]
//!
//! Row height cascades into rows: `row[height:..]` overrides
//! `table[row-height:..]`. `row[bg|color:..]` cascades into cells
//! unless the cell sets its own. Captions auto-number from the
//! per-document caption sequence so HTML and docx renders agree on
//! "Table N." enumeration given the same source.

use stem_core::ast::{Block, Body};

use super::super::ctx::HtmlCtx;
use super::super::{html_attr, html_text};
use super::table_cell::render_cell_cascaded;
use crate::style_props::{normalize_hex_color, parse_length_to_points};
use std::fmt::Write;

pub fn render_with_ctx(
    out: &mut String,
    b: &Block,
    ctx: &HtmlCtx,
) -> Result<(), std::fmt::Error> {
    let border = b.prop_str("border").unwrap_or("none");
    let stripe = b.prop_str("stripe") == Some("true");

    let mut classes = String::from("stem-table");
    let _ = write!(classes, " stem-border-{}", border_class(border));
    if stripe {
        classes.push_str(" stem-stripe");
    }

    // Wrapper styles: indent + collapse-mode plus border-collapse to
    // make `border:all|outer` paint cleanly.
    let mut style = String::from("border-collapse:collapse;");
    match border {
        "all" => style.push_str("border:1px solid currentColor;"),
        "outer" => style.push_str("border:1px solid currentColor;"),
        _ => {}
    }
    if let Some(indent_pt) = b.prop_str("indent").and_then(parse_length_to_points) {
        let _ = write!(style, "margin-left:{}pt;", fmt_pt(indent_pt));
    }

    writeln!(
        out,
        "<table class=\"{}\" data-border=\"{}\" style=\"{}\">",
        classes, border, style
    )?;

    // <colgroup> so per-column widths survive the HTML render. Bare
    // numbers in the docx `widths:` spec are dxa (a docx unit); for
    // HTML we treat them as `pt` so the values stay in a sane unit
    // family. Explicit unit suffixes (`pt`, `in`, ...) are honored.
    if let Some(widths) = b.prop_str("widths") {
        writeln!(out, "<colgroup>")?;
        for raw in widths.split(',') {
            let raw = raw.trim();
            let pt = parse_length_to_points(raw)
                .or_else(|| raw.parse::<f64>().ok().map(|n| n / 20.0));
            if let Some(pt) = pt {
                writeln!(
                    out,
                    "<col style=\"width:{};\">",
                    format!("{}pt", fmt_pt(pt))
                )?;
            } else {
                writeln!(out, "<col>")?;
            }
        }
        writeln!(out, "</colgroup>")?;
    }

    if let Some(c) = b.prop_str("caption") {
        let n = ctx.next_table_caption();
        let bookmark = format!("_Toc_table_{n}");
        writeln!(
            out,
            "<caption class=\"stem-Caption\" id=\"{}\">Table {}. {}</caption>",
            html_attr(&bookmark),
            n,
            html_text(c)
        )?;
    }

    // Row-height cascade defaults — `row[height:..]` overrides these.
    let table_row_height = b.prop_str("row-height").and_then(parse_length_to_points);
    let table_row_height_rule = b
        .prop_str("row-height-rule")
        .map(str::to_string)
        .or(Some("atLeast".to_string()));

    // Split header rows into <thead>, data rows into <tbody>, so
    // styles + accessibility tools (screen readers' table-mode) see
    // the right semantics.
    if let Body::Children(children) = &b.body {
        let rows: Vec<&Block> = children.iter().filter(|c| c.name == "row").collect();
        let mut header_emitted = false;
        let mut tbody_open = false;
        let mut data_row_idx: usize = 0;
        for row in &rows {
            let is_header = row.prop_str("kind") == Some("header");
            if is_header {
                if !header_emitted {
                    writeln!(out, "<thead>")?;
                    header_emitted = true;
                }
                render_row_with_cascade(
                    out, row, ctx, true, stripe, data_row_idx,
                    table_row_height, table_row_height_rule.as_deref(),
                )?;
            } else {
                if header_emitted && !tbody_open {
                    writeln!(out, "</thead>")?;
                }
                if !tbody_open {
                    writeln!(out, "<tbody>")?;
                    tbody_open = true;
                }
                render_row_with_cascade(
                    out, row, ctx, false, stripe, data_row_idx,
                    table_row_height, table_row_height_rule.as_deref(),
                )?;
                data_row_idx += 1;
            }
        }
        if header_emitted && !tbody_open {
            writeln!(out, "</thead>")?;
        } else if tbody_open {
            writeln!(out, "</tbody>")?;
        }
    }

    writeln!(out, "</table>")?;
    Ok(())
}

fn border_class(border: &str) -> &'static str {
    match border {
        "all" => "all",
        "outer" => "outer",
        _ => "none",
    }
}

fn render_row_with_cascade(
    out: &mut String,
    row: &Block,
    ctx: &HtmlCtx,
    is_header: bool,
    stripe: bool,
    data_row_idx: usize,
    table_row_height: Option<f64>,
    table_row_height_rule: Option<&str>,
) -> Result<(), std::fmt::Error> {
    let row_bg = row.prop_str("bg").and_then(normalize_hex_color);
    let row_color = row.prop_str("color").and_then(normalize_hex_color);
    let row_height = row
        .prop_str("height")
        .and_then(parse_length_to_points)
        .or(table_row_height);
    let row_height_rule = row
        .prop_str("height-rule")
        .or(table_row_height_rule)
        .unwrap_or("atLeast");

    let mut tr_style = String::new();
    if let Some(bg) = &row_bg {
        let _ = write!(tr_style, "background:#{};", bg);
    }
    if let Some(c) = &row_color {
        let _ = write!(tr_style, "color:#{};", c);
    }
    if let Some(h) = row_height {
        match row_height_rule {
            "exact" => {
                let _ = write!(tr_style, "height:{}pt;", fmt_pt(h));
            }
            "auto" => {}
            _ => {
                let _ = write!(tr_style, "min-height:{}pt;", fmt_pt(h));
            }
        }
    }
    // Stripe fill is applied at the row level so the cascade
    // (cell.bg > row.bg > stripe) matches the docx implementation.
    if !is_header && stripe && data_row_idx % 2 == 1 && row_bg.is_none() {
        tr_style.push_str("background:#F2F2F2;");
    }

    write!(out, "<tr")?;
    if !tr_style.is_empty() {
        write!(out, " style=\"{}\"", tr_style)?;
    }
    writeln!(out, ">")?;

    if let Body::Children(cells) = &row.body {
        for cell in cells {
            if cell.name != "cell" {
                continue;
            }
            render_cell_cascaded(out, cell, ctx, is_header, row_bg.as_deref(), row_color.as_deref())?;
        }
    }
    writeln!(out, "</tr>")?;
    Ok(())
}

fn fmt_pt(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        trimmed.to_string()
    }
}
