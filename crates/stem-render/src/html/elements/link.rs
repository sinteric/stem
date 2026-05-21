//! HTML rendering for `@link[to:..., title:...]`.
//!
//! Emits an `<a>` element. Body text pieces are escaped and concatenated;
//! inline children inside the body are not currently supported (matches
//! the prior match-arm behavior).

use stem_core::ast::Block;

use super::super::{html_attr, html_text, BodyTextPieces};
use super::HtmlElement;
use std::fmt::Write;

pub const LINK: HtmlElement = HtmlElement {
    name: "link",
    render,
};

fn render(out: &mut String, b: &Block) -> Result<(), std::fmt::Error> {
    let to = b.prop_str("to").unwrap_or("#");
    write!(out, "<a href=\"{}\"", html_attr(to))?;
    if let Some(t) = b.prop_str("title") {
        write!(out, " title=\"{}\"", html_attr(t))?;
    }
    write!(out, ">")?;
    for s in b.body_text_pieces() {
        write!(out, "{}", html_text(&s))?;
    }
    write!(out, "</a>")?;
    Ok(())
}
