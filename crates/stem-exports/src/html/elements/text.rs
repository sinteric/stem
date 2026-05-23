//! `@text` — styled inline text span.
//!
//! Mirrors the docx rPr palette: `weight` (bold/light/regular),
//! `style` (italic/normal), `decoration` (underline/strike/none),
//! `color`, `bg`, `size`, `font`. Unknown property values are
//! silently dropped — matches the ignore-unknown baseline so a
//! docx-only run attribute won't crash the HTML render.

use std::fmt::Write;

use stem_core::ast::Block;
use stem_core::theme::Theme;

use crate::style_props::{normalize_hex_color, parse_length_to_points};

use super::super::{html_text, BodyTextPieces};
use super::HtmlInlineElement;

pub const TEXT: HtmlInlineElement = HtmlInlineElement {
    name: "text",
    render,
};

fn render(out: &mut String, b: &Block, theme: &Theme) -> Result<(), std::fmt::Error> {
    let mut style = String::new();
    for p in &b.properties {
        match p.key.as_str() {
            "color" => write_color(&mut style, "color", p.value.as_str(), theme),
            "bg" => write_color(&mut style, "background", p.value.as_str(), theme),
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
            "size" => {
                if let Some(pt) = parse_length_to_points(p.value.as_str()) {
                    let _ = write!(style, "font-size:{}pt;", fmt_pt(pt));
                }
            }
            "font" => {
                let f = p.value.as_str().replace('"', "");
                let _ = write!(style, "font-family:\"{}\";", f);
            }
            _ => {}
        }
    }
    write!(out, "<span style=\"{}\">", style)?;
    for piece in &b.body_text_pieces() {
        write!(out, "{}", html_text(piece))?;
    }
    write!(out, "</span>")?;
    Ok(())
}

fn write_color(style: &mut String, css_prop: &str, value: &str, theme: &Theme) {
    // Source authors may supply either a theme-relative name
    // (`red`, `accent1`, …) or a literal hex. Try theme first so
    // documents that use named colors keep matching the theme;
    // fall back to the literal hex normalizer for raw `#RRGGBB`.
    if let Some(c) = theme.resolve_color(value) {
        let _ = write!(style, "{css_prop}:{};", c.to_hex());
        return;
    }
    if let Some(hex) = normalize_hex_color(value) {
        let _ = write!(style, "{css_prop}:#{};", hex);
    }
}

fn fmt_pt(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
