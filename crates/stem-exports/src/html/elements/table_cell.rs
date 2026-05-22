//! Document-table `cell`.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_attr, render_text_body_inline};
use super::HtmlElement;
use std::fmt::Write;

pub const CELL: HtmlElement = HtmlElement {
    name: "cell",
    render,
};

fn render(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    render_cell(out, b, theme, false)
}

pub(crate) fn render_cell(
    out: &mut String,
    b: &Block,
    theme: &Theme,
    is_header: bool,
) -> Result<(), std::fmt::Error> {
    let tag = if is_header { "th" } else { "td" };
    let mut style = String::from("padding:0.25rem 0.5rem;border:1px solid currentColor;");
    let mut attrs = String::new();
    for p in &b.properties {
        match p.key.as_str() {
            "colspan" => write!(attrs, " colspan=\"{}\"", html_attr(p.value.as_str())).unwrap(),
            "rowspan" => write!(attrs, " rowspan=\"{}\"", html_attr(p.value.as_str())).unwrap(),
            "align" => write!(style, "text-align:{};", html_attr(p.value.as_str())).unwrap(),
            "valign" => write!(style, "vertical-align:{};", html_attr(p.value.as_str())).unwrap(),
            "bg" => {
                if let Some(c) = theme.resolve_color(p.value.as_str()) {
                    write!(style, "background:{};", c.to_hex()).unwrap();
                }
            }
            _ => {}
        }
    }
    write!(out, "<{}{} style=\"{}\">", tag, attrs, style)?;
    render_text_body_inline(out, b, theme)?;
    writeln!(out, "</{}>", tag)?;
    Ok(())
}
