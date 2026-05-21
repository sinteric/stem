//! `p` — paragraph.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_attr, render_text_body_inline};
use super::HtmlElement;
use std::fmt::Write;

pub const P: HtmlElement = HtmlElement {
    name: "p",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    write!(out, "<p")?;
    if let Some(a) = b.prop_str("align") {
        write!(out, " style=\"text-align:{};\"", html_attr(a))?;
    }
    write!(out, ">")?;
    render_text_body_inline(out, b)?;
    writeln!(out, "</p>")?;
    Ok(())
}
