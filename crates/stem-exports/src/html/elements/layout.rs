//! `layout` — multi-column or sidebar layout container.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::{grid_template_for, html_attr, render_children_of};
use super::HtmlBlockElement;
use std::fmt::Write;

pub const LAYOUT: HtmlBlockElement = HtmlBlockElement {
    name: "layout",
    render,
};

fn render(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    let kind = b.prop_str("kind").unwrap_or("two-column");
    writeln!(
        out,
        "<div class=\"stem-layout\" data-layout=\"{}\" style=\"display:grid;gap:1rem;{}\">",
        html_attr(kind),
        grid_template_for(kind),
    )?;
    render_children_of(out, b, ctx)?;
    writeln!(out, "</div>")?;
    Ok(())
}
