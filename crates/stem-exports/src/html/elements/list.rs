//! `ol`, `ul`, and presentation `bullets` — list containers.
//!
//! All three share rendering. `ol` adds the `start` attribute; `ul`
//! and `bullets` render bulleted.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::{html_attr, render_children_of};
use super::HtmlBlockElement;
use std::fmt::Write;

pub const OL: HtmlBlockElement = HtmlBlockElement { name: "ol", render: render_ol };
pub const UL: HtmlBlockElement = HtmlBlockElement { name: "ul", render: render_ul };
pub const BULLETS: HtmlBlockElement = HtmlBlockElement { name: "bullets", render: render_ul };

fn render_ol(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    render_list(out, b, ctx, true)
}

fn render_ul(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    render_list(out, b, ctx, false)
}

fn render_list(
    out: &mut String,
    b: &Block,
    ctx: &HtmlCtx,
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
    render_children_of(out, b, ctx)?;
    writeln!(out, "</{}>", tag)?;
    Ok(())
}
