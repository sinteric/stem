//! `col` (document/presentation layout column).
//!
//! Sheet-side `col[at:...]` is a cascade rule consumed by the cook pass
//! and never reaches a renderer.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::render_children_of;
use super::HtmlElement;
use std::fmt::Write;

pub const COL: HtmlElement = HtmlElement {
    name: "col",
    render,
};

fn render(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    writeln!(out, "<div class=\"stem-col\">")?;
    render_children_of(out, b, theme)?;
    writeln!(out, "</div>")?;
    Ok(())
}
