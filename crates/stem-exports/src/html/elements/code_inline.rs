//! `code` inline — monospace code span.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_text, BodyTextPieces};
use super::HtmlInlineElement;
use std::fmt::Write;

pub const CODE: HtmlInlineElement = HtmlInlineElement {
    name: "code",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    let mut text = String::new();
    for s in b.body_text_pieces() {
        text.push_str(&s);
    }
    write!(out, "<code>{}</code>", html_text(&text))?;
    Ok(())
}
