//! Document-table `row`.
//!
//! Standalone `row{ cell ... }` (no enclosing `table`) renders as a
//! bare `<tr>`. Normal rows are emitted from inside `table.rs` with
//! the full cascade context.

use stem_core::ast::{Block, Body};

use super::super::ctx::HtmlCtx;
use super::table_cell;
use super::HtmlBlockElement;
use std::fmt::Write;

pub const ROW: HtmlBlockElement = HtmlBlockElement {
    name: "row",
    render,
};

fn render(out: &mut String, b: &Block, ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    let is_header = b.prop_str("kind") == Some("header");
    writeln!(out, "<tr>")?;
    if let Body::Children(children) = &b.body {
        for child in children {
            if child.name == "cell" {
                table_cell::render_cell_cascaded(out, child, ctx, is_header, None, None)?;
            }
        }
    }
    writeln!(out, "</tr>")?;
    Ok(())
}
