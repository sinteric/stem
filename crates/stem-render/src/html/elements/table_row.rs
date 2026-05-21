//! Document-table `row`.
//!
//! In top-level dispatch we render a row by itself (e.g. a stray
//! `row{ cell }` in the document). In normal use `table` reaches into
//! this module's [`render_row`] helper to pass `is_header`. Sheet rows
//! are a different concern handled by the sheet renderer.

use stem_core::ast::{Block, Body};
use stem_core::theme::Theme;

use super::{table_cell, HtmlElement};
use std::fmt::Write;

pub const ROW: HtmlElement = HtmlElement {
    name: "row",
    render,
};

fn render(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    render_row(out, b, theme, false)
}

pub(crate) fn render_row(
    out: &mut String,
    b: &Block,
    theme: &Theme,
    is_header: bool,
) -> Result<(), std::fmt::Error> {
    writeln!(out, "<tr>")?;
    if let Body::Children(children) = &b.body {
        for child in children {
            if child.name == "cell" {
                table_cell::render_cell(out, child, theme, is_header)?;
            }
        }
    }
    writeln!(out, "</tr>")?;
    Ok(())
}
