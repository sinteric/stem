//! `p` — paragraph.
//!
//! Honors the same per-paragraph property surface as the docx
//! exporter: `align`, `before`, `after`, `line`, `size`,
//! `border-top`. `tabs:` is not representable in HTML and is
//! silently ignored, matching the architecture decision that the
//! HTML renderer never errors on a docx-only property.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::render_text_body_inline;
use super::block_props::paragraph_attrs;
use super::HtmlBlockElement;
use std::fmt::Write;

pub const P: HtmlBlockElement = HtmlBlockElement {
    name: "p",
    render,
};

fn render(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    let attrs = paragraph_attrs(b);
    write!(out, "<p")?;
    if !attrs.style.is_empty() {
        write!(out, " style=\"{}\"", attrs.style)?;
    }
    write!(out, ">")?;
    render_text_body_inline(out, b, ctx.theme)?;
    writeln!(out, "</p>")?;
    Ok(())
}
