//! `@page-number` and `@total-pages` — paginated-output fields.
//!
//! HTML has no pagination, so both render as empty (a silent
//! no-op). The element registrations exist so the inline dispatch
//! doesn't emit a generic `<span data-stem="page-number">` wrapper
//! through the fallback path.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::HtmlInlineElement;

pub const PAGE_NUMBER: HtmlInlineElement = HtmlInlineElement {
    name: "page-number",
    render: render_empty,
};

pub const TOTAL_PAGES: HtmlInlineElement = HtmlInlineElement {
    name: "total-pages",
    render: render_empty,
};

fn render_empty(_out: &mut String, _b: &Block, _theme: &Theme) -> Result<(), std::fmt::Error> {
    Ok(())
}
