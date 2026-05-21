//! `math` inline — math notation placeholder.
//!
//! Currently emits the body as text in a tagged span. A future change
//! will add a `notation` property (`latex`/`asciimath`/`mathml`) and
//! emit proper MathML.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_text, BodyTextPieces};
use super::HtmlElement;
use std::fmt::Write;

pub const MATH: HtmlElement = HtmlElement {
    name: "math",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    let mut text = String::new();
    for s in b.body_text_pieces() {
        text.push_str(&s);
    }
    write!(out, "<span class=\"stem-math\">{}</span>", html_text(&text))?;
    Ok(())
}
