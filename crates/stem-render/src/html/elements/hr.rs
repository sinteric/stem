//! `hr` — horizontal rule.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::HtmlElement;
use std::fmt::Write;

pub const HR: HtmlElement = HtmlElement {
    name: "hr",
    render,
};

fn render(out: &mut String, _b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    writeln!(out, "<hr>")?;
    Ok(())
}
