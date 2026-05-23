//! `hr` — horizontal rule.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::HtmlBlockElement;
use std::fmt::Write;

pub const HR: HtmlBlockElement = HtmlBlockElement {
    name: "hr",
    render,
};

fn render(out: &mut String, _b: &Block, _ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    writeln!(out, "<hr>")?;
    Ok(())
}
