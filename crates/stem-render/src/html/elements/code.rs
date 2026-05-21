//! `code` block — preformatted code listing with language hint.

use stem_core::ast::{Block, Body, TextPiece};
use stem_core::theme::Theme;

use super::super::{html_attr, html_text};
use super::HtmlElement;
use std::fmt::Write;

pub const CODE: HtmlElement = HtmlElement {
    name: "code",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    let lang = b.prop_str("lang").unwrap_or("");
    write!(out, "<pre><code class=\"language-{}\">", html_attr(lang))?;
    if let Body::Text(pieces) = &b.body {
        for p in pieces {
            if let TextPiece::Literal { text, .. } = p {
                write!(out, "{}", html_text(text))?;
            }
        }
    }
    writeln!(out, "</code></pre>")?;
    Ok(())
}
