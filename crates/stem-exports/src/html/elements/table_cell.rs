//! Document-table `cell`.
//!
//! Two entry points:
//! - [`CELL`] in the dispatch table — standalone `cell` (no table
//!   parent). Renders without cascade.
//! - [`render_cell_cascaded`] — used by `table.rs` to thread row
//!   bg/color into the cell's style when the cell hasn't set its
//!   own.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::{html_attr, render_text_body_inline};
use super::HtmlBlockElement;
use crate::style_props::normalize_hex_color;
use std::fmt::Write;

pub const CELL: HtmlBlockElement = HtmlBlockElement {
    name: "cell",
    render,
};

fn render(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    render_cell_cascaded(out, b, ctx, false, None, None)
}

pub(crate) fn render_cell_cascaded(
    out: &mut String,
    b: &Block,
    ctx: &HtmlCtx,
    is_header: bool,
    row_bg: Option<&str>,
    row_color: Option<&str>,
) -> Result<(), std::fmt::Error> {
    let theme = ctx.theme;
    let tag = if is_header { "th" } else { "td" };
    let mut style = String::from("padding:0.25rem 0.5rem;border:1px solid currentColor;");
    let mut attrs = String::new();

    // Cell properties override row cascade.
    let mut cell_bg_set = false;
    let mut cell_color_set = false;

    for p in &b.properties {
        match p.key.as_str() {
            "colspan" => write!(attrs, " colspan=\"{}\"", html_attr(p.value.as_str())).unwrap(),
            "rowspan" => write!(attrs, " rowspan=\"{}\"", html_attr(p.value.as_str())).unwrap(),
            "align" => write!(style, "text-align:{};", html_attr(p.value.as_str())).unwrap(),
            "valign" => write!(
                style,
                "vertical-align:{};",
                map_valign(p.value.as_str())
            )
            .unwrap(),
            "bg" => {
                let hex = normalize_hex_color(p.value.as_str());
                if let Some(h) = hex {
                    write!(style, "background:#{};", h).unwrap();
                    cell_bg_set = true;
                } else if let Some(c) = theme.resolve_color(p.value.as_str()) {
                    write!(style, "background:{};", c.to_hex()).unwrap();
                    cell_bg_set = true;
                }
            }
            "color" => {
                let hex = normalize_hex_color(p.value.as_str());
                if let Some(h) = hex {
                    write!(style, "color:#{};", h).unwrap();
                    cell_color_set = true;
                } else if let Some(c) = theme.resolve_color(p.value.as_str()) {
                    write!(style, "color:{};", c.to_hex()).unwrap();
                    cell_color_set = true;
                }
            }
            _ => {}
        }
    }
    // Row cascade kicks in only when the cell didn't claim the
    // property — matches docx's row→cell precedence rule.
    if !cell_bg_set {
        if let Some(bg) = row_bg {
            write!(style, "background:#{};", bg).unwrap();
        }
    }
    if !cell_color_set {
        if let Some(c) = row_color {
            write!(style, "color:#{};", c).unwrap();
        }
    }

    write!(out, "<{}{} style=\"{}\">", tag, attrs, style)?;
    render_text_body_inline(out, b, theme)?;
    writeln!(out, "</{}>", tag)?;
    Ok(())
}

fn map_valign(s: &str) -> &str {
    match s {
        "top" => "top",
        "middle" | "center" => "middle",
        "bottom" => "bottom",
        _ => s,
    }
}
