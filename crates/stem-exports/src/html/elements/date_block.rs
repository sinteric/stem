//! `date` as a block element (the inline form lives in `date_inline`).

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::render_text_body_inline;
use super::HtmlBlockElement;
use std::fmt::Write;

pub const DATE: HtmlBlockElement = HtmlBlockElement {
    name: "date",
    render,
};

fn render(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    write!(out, "<time>")?;
    render_text_body_inline(out, b, ctx.theme)?;
    writeln!(out, "</time>")?;
    Ok(())
}
