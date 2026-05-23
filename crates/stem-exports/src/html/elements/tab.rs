//! `@tab` — explicit tab stop.
//!
//! HTML has no flow-tab semantics, so the closest visible
//! approximation is an em-space. `tabs:` on the wrapping paragraph
//! is silently dropped — HTML can't honor flow tab stops.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::HtmlInlineElement;
use std::fmt::Write;

pub const TAB: HtmlInlineElement = HtmlInlineElement {
    name: "tab",
    render,
};

fn render(out: &mut String, _b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    write!(out, "&emsp;")?;
    Ok(())
}
