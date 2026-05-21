//! `date` as a block element (the inline form lives in `date_inline`).

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::render_text_body_inline;
use super::HtmlElement;
use std::fmt::Write;

pub const DATE: HtmlElement = HtmlElement {
    name: "date",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    write!(out, "<time>")?;
    render_text_body_inline(out, b)?;
    writeln!(out, "</time>")?;
    Ok(())
}
