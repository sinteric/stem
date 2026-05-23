//! `pagebreak` — explicit page break for paginated output.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::HtmlBlockElement;
use std::fmt::Write;

pub const PAGEBREAK: HtmlBlockElement = HtmlBlockElement {
    name: "pagebreak",
    render,
};

fn render(out: &mut String, _b: &Block, _ctx: &HtmlCtx) -> Result<(), std::fmt::Error> {
    writeln!(
        out,
        "<div class=\"stem-pagebreak\" style=\"page-break-after:always;\"></div>"
    )?;
    Ok(())
}
