//! Per-element HTML render functions.
//!
//! Each submodule owns the HTML rendering for one element. The
//! [`INLINE_RENDERERS`] and [`BLOCK_RENDERERS`] consts collect them
//! into dispatch tables consulted by [`super::render_inline_dispatch`]
//! and [`super::render_block_dispatch`] before the legacy match arms.
//!
//! Migration: elements move here one at a time. Once moved, the
//! corresponding match arm is deleted from the parent file. When the
//! last element migrates, both match arms become empty and are removed.

use stem_core::ast::Block;

pub mod link;

/// Function-pointer signature for an element's HTML render.
///
/// Returns `std::fmt::Error` to match the existing render_inline /
/// render_block chain; the top-level `HtmlRenderer::render` wraps this
/// in `HtmlError` at the boundary.
pub type HtmlFn = fn(&mut String, &Block) -> Result<(), std::fmt::Error>;

#[derive(Clone, Copy, Debug)]
pub struct HtmlElement {
    pub name: &'static str,
    pub render: HtmlFn,
}

pub const INLINE_RENDERERS: &[&HtmlElement] = &[&link::LINK];

pub const BLOCK_RENDERERS: &[&HtmlElement] = &[];

/// Look up an inline renderer by element name.
pub fn lookup_inline(name: &str) -> Option<&'static HtmlElement> {
    INLINE_RENDERERS.iter().copied().find(|e| e.name == name)
}

/// Look up a block renderer by element name.
pub fn lookup_block(name: &str) -> Option<&'static HtmlElement> {
    BLOCK_RENDERERS.iter().copied().find(|e| e.name == name)
}
