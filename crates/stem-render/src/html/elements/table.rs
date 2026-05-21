//! `table` — document-table container.
//!
//! Sheet `sheet` is a different element entirely; this one is the
//! document/presentation table.

use stem_core::ast::{Block, Body};
use stem_core::theme::Theme;

use super::super::html_text;
use super::{table_row, HtmlElement};
use std::fmt::Write;

pub const TABLE: HtmlElement = HtmlElement {
    name: "table",
    render,
};

fn render(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let border = b.prop_str("border").unwrap_or("none");
    let style = match border {
        "outer" | "all" => "border:1px solid currentColor;border-collapse:collapse;",
        _ => "border-collapse:collapse;",
    };
    writeln!(out, "<table data-border=\"{}\" style=\"{}\">", border, style)?;
    if let Some(c) = b.prop_str("caption") {
        writeln!(out, "<caption>{}</caption>", html_text(c))?;
    }
    if let Body::Children(children) = &b.body {
        for child in children {
            if child.name == "row" {
                let is_header = child.prop_str("kind") == Some("header");
                table_row::render_row(out, child, theme, is_header)?;
            }
        }
    }
    writeln!(out, "</table>")?;
    Ok(())
}
