//! `speaker-note` — hidden presenter notes for slides.

use stem_core::ast::{Block, Body, TextPiece};

use super::super::ctx::HtmlCtx;
use super::super::html_text;
use super::HtmlBlockElement;
use std::fmt::Write;

pub const SPEAKER_NOTE: HtmlBlockElement = HtmlBlockElement {
    name: "speaker-note",
    render,
};

fn render(out: &mut String, b: &Block, _ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
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
