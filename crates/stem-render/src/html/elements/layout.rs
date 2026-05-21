//! `layout` — multi-column or sidebar layout container.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{grid_template_for, html_attr, render_children_of};
use super::HtmlElement;
use std::fmt::Write;

pub const LAYOUT: HtmlElement = HtmlElement {
    name: "layout",
    render,
};

fn render(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let kind = b.prop_str("kind").unwrap_or("two-column");
    writeln!(
        out,
        "<div class=\"stem-layout\" data-layout=\"{}\" style=\"display:grid;gap:1rem;{}\">",
        html_attr(kind),
        grid_template_for(kind),
    )?;
    render_children_of(out, b, theme)?;
    writeln!(out, "</div>")?;
    Ok(())
}
