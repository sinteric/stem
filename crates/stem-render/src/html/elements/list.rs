//! `ol`, `ul`, and presentation `bullets` — list containers.
//!
//! All three share rendering. `ol` adds the `start` attribute; `ul`
//! and `bullets` render bulleted.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_attr, render_children_of};
use super::HtmlElement;
use std::fmt::Write;

pub const OL: HtmlElement = HtmlElement { name: "ol", render: render_ol };
pub const UL: HtmlElement = HtmlElement { name: "ul", render: render_ul };
pub const BULLETS: HtmlElement = HtmlElement { name: "bullets", render: render_ul };

fn render_ol(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    render_list(out, b, theme, true)
}

fn render_ul(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    render_list(out, b, theme, false)
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
        write!(out, " data-style=\"{}\"", html_attr(style))?;
    }
    writeln!(out, ">")?;
    render_children_of(out, b, theme)?;
    writeln!(out, "</{}>", tag)?;
    Ok(())
}
