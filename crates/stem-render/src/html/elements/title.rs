//! `title` — slide title.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::render_text_body_inline;
use super::HtmlElement;
use std::fmt::Write;

pub const TITLE: HtmlElement = HtmlElement {
    name: "title",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    write!(out, "<h1 class=\"stem-slide-title\">")?;
    render_text_body_inline(out, b)?;
    writeln!(out, "</h1>")?;
    Ok(())
}
