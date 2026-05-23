//! `title` — paper/presentation title block.
//!
//! Renders as `<h1 class="stem-Title">` so per-document
//! `style[id:Title, ...]` overrides can patch it through the same
//! CSS-class mechanism the docx exporter uses for the Title style.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::render_text_body_inline;
use super::block_props::paragraph_attrs;
use super::HtmlBlockElement;
use std::fmt::Write;

pub const TITLE: HtmlBlockElement = HtmlBlockElement {
    name: "title",
    render,
};

fn render(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    let attrs = paragraph_attrs(b);
    write!(out, "<h1 class=\"stem-Title\"")?;
    if !attrs.style.is_empty() {
        write!(out, " style=\"{}\"", attrs.style)?;
    }
    write!(out, ">")?;
    render_text_body_inline(out, b, ctx.theme)?;
    writeln!(out, "</h1>")?;
    Ok(())
}
