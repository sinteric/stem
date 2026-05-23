//! Per-element HTML render functions.
//!
//! Each submodule owns the HTML rendering for one element. The
//! [`INLINE_RENDERERS`] and [`BLOCK_RENDERERS`] consts collect them
//! into dispatch tables consulted by [`super::render_inline`] and
//! [`super::render_block`].
//!
//! Convention: per-element module is named after the element. Where one
//! name spans an inline form AND a block form (e.g. `code`, `date`,
//! `math`), the inline file gets the `_inline` suffix; the bare name is
//! the block form. For `cell`/`row` the block table_* prefix
//! disambiguates the document-table form from the sheet form (which is
//! rendered internally by `sheet`).
//!
//! ## Block vs inline signatures
//!
//! Block renderers take `&HtmlCtx` (which carries the theme along
//! with per-document context like style overrides and heading/caption
//! anchors). Inline renderers take just `&Theme` — inline elements
//! don't reach into document-level state.
//!
//! Elements that need ctx access for their own logic (heading
//! bookmarks, image/table caption numbering, TOC sections) are
//! intercepted in [`super::render_block`] before dispatch and aren't
//! listed in [`BLOCK_RENDERERS`]; their module-level fns live next
//! to the rest of the renderers but expose a `render_with_ctx`
//! entrypoint instead.

use stem_core::ast::Block;
use stem_core::theme::Theme;

use super::ctx::HtmlCtx;

// --- shared helpers ---
pub mod block_props;

// --- block ---
pub mod blockquote;
pub mod code;
pub mod col;
pub mod date_block;
pub mod heading;
pub mod hr;
pub mod image;
pub mod layout;
pub mod list;
pub mod list_item;
pub mod note;
pub mod pagebreak;
pub mod paragraph;
pub mod section;
pub mod sheet;
pub mod slide;
pub mod speaker_note;
pub mod table;
pub mod table_cell;
pub mod table_row;
pub mod title;

// --- inline ---
pub mod br;
pub mod code_inline;
pub mod date_inline;
pub mod footnote;
pub mod link;
pub mod math_inline;
pub mod mention;
pub mod page_field;
pub mod tab;
pub mod text;

/// Function-pointer signature for a block element's HTML render.
pub type HtmlBlockFn = fn(&mut String, &Block, &HtmlCtx) -> Result<(), std::fmt::Error>;

/// Function-pointer signature for an inline element's HTML render.
pub type HtmlInlineFn = fn(&mut String, &Block, &Theme) -> Result<(), std::fmt::Error>;

#[derive(Clone, Copy, Debug)]
pub struct HtmlBlockElement {
    pub name: &'static str,
    pub render: HtmlBlockFn,
}

#[derive(Clone, Copy, Debug)]
pub struct HtmlInlineElement {
    pub name: &'static str,
    pub render: HtmlInlineFn,
}

pub const INLINE_RENDERERS: &[&HtmlInlineElement] = &[
    &br::BR,
    &code_inline::CODE,
    &date_inline::DATE,
    &footnote::FOOTNOTE,
    &link::LINK,
    &math_inline::MATH,
    &mention::MENTION,
    &page_field::PAGE_NUMBER,
    &page_field::TOTAL_PAGES,
    &tab::TAB,
    &text::TEXT,
];

pub const BLOCK_RENDERERS: &[&HtmlBlockElement] = &[
    &blockquote::BLOCKQUOTE,
    &code::CODE,
    &col::COL,
    &date_block::DATE,
    &hr::HR,
    &layout::LAYOUT,
    &list::OL,
    &list::UL,
    &list::BULLETS,
    &list_item::LI,
    &list_item::ITEM,
    &note::NOTE,
    &pagebreak::PAGEBREAK,
    &paragraph::P,
    &sheet::SHEET,
    &slide::SLIDE,
    &speaker_note::SPEAKER_NOTE,
    &table_cell::CELL,
    &table_row::ROW,
    &title::TITLE,
];

/// Look up an inline renderer by element name.
pub fn lookup_inline(name: &str) -> Option<&'static HtmlInlineElement> {
    INLINE_RENDERERS.iter().copied().find(|e| e.name == name)
}

/// Look up a block renderer by element name. Elements intercepted
/// in [`super::render_block`] (`h1..h6`, `image`, `table`,
/// `section`, `style`, `header`, `footer`) deliberately aren't in
/// the table — the intercept handles them with full ctx access.
pub fn lookup_block(name: &str) -> Option<&'static HtmlBlockElement> {
    BLOCK_RENDERERS.iter().copied().find(|e| e.name == name)
}
