//! `li` (list item) and presentation `item` — same rendering.

use stem_core::ast::{Block, Body};

use super::super::ctx::HtmlCtx;
use super::super::{html_attr, render_children_of, render_text_body_inline};
use super::HtmlBlockElement;
use std::fmt::Write;

pub const LI: HtmlBlockElement = HtmlBlockElement { name: "li", render };
pub const ITEM: HtmlBlockElement = HtmlBlockElement { name: "item", render };

fn render(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    write!(out, "<li")?;
    if let Some(at) = b.prop_str("at") {
        write!(out, " value=\"{}\"", html_attr(at))?;
    }
    write!(out, ">")?;
    match &b.body {
        Body::Text(_) => render_text_body_inline(out, b, ctx.theme)?,
        Body::Children(_) => render_children_of(out, b, ctx)?,
        Body::None => {}
    }
    writeln!(out, "</li>")?;
    Ok(())
}
