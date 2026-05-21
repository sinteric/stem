//! `pagebreak` — explicit page break for paginated output.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::HtmlElement;
use std::fmt::Write;

pub const PAGEBREAK: HtmlElement = HtmlElement {
    name: "pagebreak",
    render,
};

fn render(out: &mut String, _b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    writeln!(
        out,
        "<div class=\"stem-pagebreak\" style=\"page-break-after:always;\"></div>"
    )?;
    Ok(())
}
