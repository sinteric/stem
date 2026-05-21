//! `blockquote` — quoted prose block.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_attr, render_text_body_inline};
use super::HtmlElement;
use std::fmt::Write;

pub const BLOCKQUOTE: HtmlElement = HtmlElement {
    name: "blockquote",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    write!(out, "<blockquote")?;
    if let Some(c) = b.prop_str("cite") {
        write!(out, " cite=\"{}\"", html_attr(c))?;
    }
    write!(out, ">")?;
    render_text_body_inline(out, b)?;
    writeln!(out, "</blockquote>")?;
    Ok(())
}
