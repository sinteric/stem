//! `section` — semantic document section.
//!
//! Special case: a marker `section[id:toc]` with no body renders the
//! table-of-contents nav placeholder.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_attr, render_children_of};
use super::HtmlElement;
use std::fmt::Write;

pub const SECTION: HtmlElement = HtmlElement {
    name: "section",
    render,
};

fn render(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let id = b.prop_str("id");
    match id {
        Some(id) => writeln!(out, "<section data-id=\"{}\">", html_attr(id))?,
        None => writeln!(out, "<section>")?,
    }
    if id == Some("toc") && b.body.is_none() {
        writeln!(
            out,
            "<nav class=\"stem-toc\" aria-label=\"Table of contents\"><!-- generated --></nav>"
        )?;
    } else {
        render_children_of(out, b, theme)?;
    }
    writeln!(out, "</section>")?;
    Ok(())
}
