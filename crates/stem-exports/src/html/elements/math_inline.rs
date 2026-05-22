//! `math` inline — converts the body to MathML via the chosen notation.
//!
//! Properties:
//! - `notation`: `latex` (default), `asciimath`, `mathml`. Only `latex`
//!   and `mathml` (pass-through) are implemented.
//! - `display`: `inline` (default) or `block`.
//!
//! Render-time errors produce a tagged error span; the validate hook in
//! `stem_types::elements::math` catches most issues earlier.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::super::{html_attr, BodyTextPieces};
use super::HtmlElement;
use crate::math::to_mathml;
use std::fmt::Write;

pub const MATH: HtmlElement = HtmlElement {
    name: "math",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    let mut src = String::new();
    for s in b.body_text_pieces() {
        src.push_str(&s);
    }
    let notation = b.prop_str("notation").unwrap_or("latex");
    let block = b.prop_str("display") == Some("block");
    let cls = if block { "stem-math block" } else { "stem-math inline" };
    match to_mathml(&src, notation, block) {
        Ok(mathml) => {
            write!(out, "<span class=\"{}\">{}</span>", cls, mathml)?;
        }
        Err(e) => {
            write!(
                out,
                "<span class=\"stem-math-error\" title=\"{}\">[math error]</span>",
                html_attr(&e.to_string())
            )?;
        }
    }
    Ok(())
}
