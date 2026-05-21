//! `h1`..`h6` — section headings.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_attr, render_text_body_inline};
use super::HtmlElement;
use std::fmt::Write;

pub const H1: HtmlElement = HtmlElement { name: "h1", render: render_h1 };
pub const H2: HtmlElement = HtmlElement { name: "h2", render: render_h2 };
pub const H3: HtmlElement = HtmlElement { name: "h3", render: render_h3 };
pub const H4: HtmlElement = HtmlElement { name: "h4", render: render_h4 };
pub const H5: HtmlElement = HtmlElement { name: "h5", render: render_h5 };
pub const H6: HtmlElement = HtmlElement { name: "h6", render: render_h6 };

fn render_h1(out: &mut String, b: &Block, t: &Theme) -> Result<(), std::fmt::Error> { render_heading(out, b, t, 1) }
fn render_h2(out: &mut String, b: &Block, t: &Theme) -> Result<(), std::fmt::Error> { render_heading(out, b, t, 2) }
fn render_h3(out: &mut String, b: &Block, t: &Theme) -> Result<(), std::fmt::Error> { render_heading(out, b, t, 3) }
fn render_h4(out: &mut String, b: &Block, t: &Theme) -> Result<(), std::fmt::Error> { render_heading(out, b, t, 4) }
fn render_h5(out: &mut String, b: &Block, t: &Theme) -> Result<(), std::fmt::Error> { render_heading(out, b, t, 5) }
fn render_h6(out: &mut String, b: &Block, t: &Theme) -> Result<(), std::fmt::Error> { render_heading(out, b, t, 6) }

fn render_heading(out: &mut String, b: &Block, theme: &Theme, level: u8) -> Result<(), std::fmt::Error> {
    write!(out, "<h{}", level)?;
    if let Some(id) = b.prop_str("id") {
        write!(out, " id=\"{}\"", html_attr(id))?;
    }
    write!(out, ">")?;
    render_text_body_inline(out, b, theme)?;
    writeln!(out, "</h{}>", level)?;
    Ok(())
}
