//! `note` — sidebar/callout (info/warning/etc).

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::{html_attr, render_text_body_inline};
use super::HtmlBlockElement;
use std::fmt::Write;

pub const NOTE: HtmlBlockElement = HtmlBlockElement {
    name: "note",
    render,
};

fn render(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    let theme = ctx.theme;
    let bg = theme
        .resolve_color("gray")
        .map(|c| c.to_hex())
        .unwrap_or_else(|| "#f6f8fa".into());
    let kind = b.prop_str("kind").unwrap_or("info");
    writeln!(
        out,
        "<aside class=\"stem-note stem-note-{}\" style=\"display:block;padding:0.5rem 0.75rem;\
         background:{};border-left:3px solid #8b949e;margin:1rem 0;\">",
        html_attr(kind),
        bg
    )?;
    render_text_body_inline(out, b, theme)?;
    writeln!(out, "</aside>")?;
    Ok(())
}
