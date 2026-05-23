//! `slide` — a presentation slide.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::{html_attr, render_children_of};
use super::HtmlBlockElement;
use std::fmt::Write;

pub const SLIDE: HtmlBlockElement = HtmlBlockElement {
    name: "slide",
    render,
};

fn render(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    let theme = ctx.theme;
    let mut style = String::from(
        "page-break-after:always;min-height:5in;padding:1rem;border:1px dashed #aaa;\
         margin-bottom:1rem;",
    );
    if let Some(bg) = b.prop_str("background") {
        if let Some(c) = theme.resolve_color(bg) {
            write!(style, "background:{};", c.to_hex()).unwrap();
        }
    }
    let id = b.prop_str("id").unwrap_or("");
    let layout = b.prop_str("layout").unwrap_or("");
    writeln!(
        out,
        "<section class=\"stem-slide\" data-id=\"{}\" data-layout=\"{}\" style=\"{}\">",
        html_attr(id),
        html_attr(layout),
        style,
    )?;
    render_children_of(out, b, ctx)?;
    writeln!(out, "</section>")?;
    Ok(())
}
