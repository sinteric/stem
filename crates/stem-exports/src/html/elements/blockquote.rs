//! `blockquote` — quoted prose block.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::{html_attr, render_text_body_inline};
use super::block_props::{paragraph_attrs, ParagraphAttrs};
use super::HtmlBlockElement;
use std::fmt::Write;

pub const BLOCKQUOTE: HtmlBlockElement = HtmlBlockElement {
    name: "blockquote",
    render,
};

fn render(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    let ParagraphAttrs { style, .. } = paragraph_attrs(b);
    write!(out, "<blockquote")?;
    if let Some(c) = b.prop_str("cite") {
        write!(out, " cite=\"{}\"", html_attr(c))?;
    }
    if !style.is_empty() {
        write!(out, " style=\"{}\"", style)?;
    }
    write!(out, ">")?;
    render_text_body_inline(out, b, ctx.theme)?;
    writeln!(out, "</blockquote>")?;
    Ok(())
}
