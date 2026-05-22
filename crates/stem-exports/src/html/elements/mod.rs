//! Per-element HTML render functions.
//!
//! Each submodule owns the HTML rendering for one element. The
//! [`INLINE_RENDERERS`] and [`BLOCK_RENDERERS`] consts collect them
//! into dispatch tables consulted by [`super::render_inline`] and
//! [`super::render_block`] before the legacy match arms (which are
//! now empty for migrated elements).
//!
//! Convention: per-element module is named after the element. Where one
//! name spans an inline form AND a block form (e.g. `code`, `date`,
//! `math`), the inline file gets the `_inline` suffix; the bare name is
//! the block form. For `cell`/`row` the block table_* prefix
//! disambiguates the document-table form from the sheet form (which is
//! rendered internally by `sheet`).

use stem_core::ast::Block;
use stem_core::theme::Theme;

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
pub mod code_inline;
pub mod date_inline;
pub mod footnote;
pub mod link;
pub mod math_inline;
pub mod mention;
pub mod text;

/// Function-pointer signature for an element's HTML render.
///
/// Returns `std::fmt::Error` to match the existing render_inline /
/// render_block chain; the top-level `HtmlExporter::export` wraps this
/// in `HtmlError` at the boundary.
pub type HtmlFn = fn(&mut String, &Block, &Theme) -> Result<(), std::fmt::Error>;

#[derive(Clone, Copy, Debug)]
pub struct HtmlElement {
    pub name: &'static str,
    pub render: HtmlFn,
}

pub const INLINE_RENDERERS: &[&HtmlElement] = &[
    &code_inline::CODE,
    &date_inline::DATE,
    &footnote::FOOTNOTE,
    &link::LINK,
    &math_inline::MATH,
    &mention::MENTION,
    &text::TEXT,
];

pub const BLOCK_RENDERERS: &[&HtmlElement] = &[
    &blockquote::BLOCKQUOTE,
    &code::CODE,
    &col::COL,
    &date_block::DATE,
    &heading::H1,
    &heading::H2,
    &heading::H3,
    &heading::H4,
    &heading::H5,
    &heading::H6,
    &hr::HR,
    &image::IMAGE,
    &layout::LAYOUT,
    &list::OL,
    &list::UL,
    &list::BULLETS,
    &list_item::LI,
    &list_item::ITEM,
    &note::NOTE,
    &pagebreak::PAGEBREAK,
    &paragraph::P,
    &section::SECTION,
    &sheet::SHEET,
    &slide::SLIDE,
    &speaker_note::SPEAKER_NOTE,
    &table::TABLE,
    &table_cell::CELL,
    &table_row::ROW,
    &title::TITLE,
];

/// Look up an inline renderer by element name.
pub fn lookup_inline(name: &str) -> Option<&'static HtmlElement> {
    INLINE_RENDERERS.iter().copied().find(|e| e.name == name)
}

/// Look up a block renderer by element name.
pub fn lookup_block(name: &str) -> Option<&'static HtmlElement> {
    BLOCK_RENDERERS.iter().copied().find(|e| e.name == name)
}
