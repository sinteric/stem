//! `date` inline — semantic date span.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_text, BodyTextPieces};
use super::HtmlElement;
use std::fmt::Write;

pub const DATE: HtmlElement = HtmlElement {
    name: "date",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    let mut text = String::new();
    for s in b.body_text_pieces() {
        text.push_str(&s);
    }
    write!(out, "<time>{}</time>", html_text(&text))?;
    Ok(())
}
