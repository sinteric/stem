//! `image` — figure with optional caption.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_attr, html_text};
use super::HtmlElement;
use std::fmt::Write;

pub const IMAGE: HtmlElement = HtmlElement {
    name: "image",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    let src = b.prop_str("src").unwrap_or("");
    let alt = b.prop_str("alt").unwrap_or("");
    write!(
        out,
        "<figure><img src=\"{}\" alt=\"{}\"",
        html_attr(src),
        html_attr(alt)
    )?;
    if let Some(w) = b.prop_str("w") {
        write!(out, " style=\"width:{};\"", html_attr(w))?;
    }
    writeln!(out, ">")?;
    if let Some(c) = b.prop_str("caption") {
        writeln!(out, "<figcaption>{}</figcaption>", html_text(c))?;
    }
    writeln!(out, "</figure>")?;
    Ok(())
}
