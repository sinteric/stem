//! `speaker-note` — hidden presenter notes for slides.

use stem_core::ast::{Block, Body, TextPiece};
use stem_core::theme::Theme;

use super::super::html_text;
use super::HtmlElement;
use std::fmt::Write;

pub const SPEAKER_NOTE: HtmlElement = HtmlElement {
    name: "speaker-note",
    render,
};

fn render(out: &mut String, b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    write!(
        out,
        "<aside class=\"stem-speaker-note\" hidden style=\"display:none;\">"
    )?;
    if let Body::Text(pieces) = &b.body {
        for p in pieces {
            if let TextPiece::Literal { text, .. } = p {
                write!(out, "{}", html_text(text))?;
            }
        }
    }
    writeln!(out, "</aside>")?;
    Ok(())
}
