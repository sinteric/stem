//! `h1`..`h6` — section headings.
//!
//! Intercepted in [`super::super::render_block`] so the renderer can
//! stamp a `_Toc<n>` bookmark id (allocated by the prepass) on the
//! heading. Authors writing `@link[to:"ref://_Toc1"]` can then point
//! at headings stably, matching the docx exporter's anchor naming.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::{html_attr, render_text_body_inline};
use super::block_props::paragraph_attrs;
use std::fmt::Write;

pub fn render_with_ctx(
    out: &mut String,
    b: &Block,
    ctx: &HtmlCtx,
    level: u8,
) -> Result<(), std::fmt::Error> {
    let anchor = ctx.next_heading_anchor();
    let attrs = paragraph_attrs(b);
    write!(out, "<h{level} class=\"stem-Heading{level}\"")?;
    // `id:` on the source overrides the prepass bookmark — useful
    // when an author wants a friendly URL fragment.
    let id = b
        .prop_str("id")
        .map(str::to_string)
        .or_else(|| anchor.map(|a| a.bookmark.clone()));
    if let Some(id) = id {
        write!(out, " id=\"{}\"", html_attr(&id))?;
    }
    if !attrs.style.is_empty() {
        write!(out, " style=\"{}\"", attrs.style)?;
    }
    write!(out, ">")?;
    render_text_body_inline(out, b, ctx.theme)?;
    writeln!(out, "</h{level}>")?;
    Ok(())
}
