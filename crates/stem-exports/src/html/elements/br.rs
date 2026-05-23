//! `@br` — explicit line break.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::HtmlInlineElement;
use std::fmt::Write;

pub const BR: HtmlInlineElement = HtmlInlineElement {
    name: "br",
    render,
};

fn render(out: &mut String, _b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    write!(out, "<br>")?;
    Ok(())
}
