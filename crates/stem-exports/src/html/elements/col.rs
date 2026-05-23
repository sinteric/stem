//! `col` (document/presentation layout column).
//!
//! Sheet-side `col[at:...]` is a cascade rule consumed by the cook pass
//! and never reaches a renderer.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::render_children_of;
use super::HtmlBlockElement;
use std::fmt::Write;

pub const COL: HtmlBlockElement = HtmlBlockElement {
    name: "col",
    render,
};

fn render(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    writeln!(out, "<div class=\"stem-col\">")?;
    render_children_of(out, b, ctx)?;
    writeln!(out, "</div>")?;
    Ok(())
}
