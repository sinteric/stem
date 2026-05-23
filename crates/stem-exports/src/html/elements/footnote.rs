//! `footnote` inline — superscript marker with the note as tooltip.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_attr, BodyTextPieces};
use super::HtmlInlineElement;
use std::fmt::Write;

pub const FOOTNOTE: HtmlInlineElement = HtmlInlineElement {
    name: "footnote",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    let mut text = String::new();
    for s in b.body_text_pieces() {
        text.push_str(&s);
    }
    write!(
        out,
        "<sup class=\"stem-footnote\" title=\"{}\">*</sup>",
        html_attr(&text)
    )?;
    Ok(())
}
