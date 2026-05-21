//! `li` (list item) and presentation `item` — same rendering.

use stem_core::ast::{Block, Body};
use stem_core::theme::Theme;

use super::super::{html_attr, render_children_of, render_text_body_inline};
use super::HtmlElement;
use std::fmt::Write;

pub const LI: HtmlElement = HtmlElement { name: "li", render };
pub const ITEM: HtmlElement = HtmlElement { name: "item", render };

fn render(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    write!(out, "<li")?;
    if let Some(at) = b.prop_str("at") {
        write!(out, " value=\"{}\"", html_attr(at))?;
    }
    write!(out, ">")?;
    match &b.body {
        Body::Text(_) => render_text_body_inline(out, b, theme)?,
        Body::Children(_) => render_children_of(out, b, theme)?,
        Body::None => {}
    }
    writeln!(out, "</li>")?;
    Ok(())
}
