//! `text` — styled inline text span.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_text, BodyTextPieces};
use super::HtmlElement;
use std::fmt::Write;

pub const TEXT: HtmlElement = HtmlElement {
    name: "text",
    render,
};

fn render(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let mut style = String::new();
    for p in &b.properties {
        match p.key.as_str() {
            "color" => {
                if let Some(c) = theme.resolve_color(p.value.as_str()) {
                    write!(style, "color:{};", c.to_hex()).unwrap();
                }
            }
            "bg" => {
                if let Some(c) = theme.resolve_color(p.value.as_str()) {
                    write!(style, "background:{};", c.to_hex()).unwrap();
                }
            }
            "weight" => match p.value.as_str() {
                "light" => style.push_str("font-weight:300;"),
                "regular" => style.push_str("font-weight:400;"),
                "bold" => style.push_str("font-weight:700;"),
                _ => {}
            },
            "style" => match p.value.as_str() {
                "italic" | "oblique" => style.push_str("font-style:italic;"),
                "normal" => style.push_str("font-style:normal;"),
                _ => {}
            },
            "decoration" => match p.value.as_str() {
                "underline" => style.push_str("text-decoration:underline;"),
                "strike" => style.push_str("text-decoration:line-through;"),
                "none" => style.push_str("text-decoration:none;"),
                _ => {}
            },
            _ => {}
        }
    }
    write!(out, "<span style=\"{}\">", style)?;
    for p in &b.body_text_pieces() {
        write!(out, "{}", html_text(p))?;
    }
    write!(out, "</span>")?;
    Ok(())
}
