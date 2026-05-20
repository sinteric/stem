//! Stem parser. Turns source text into a [`stem_core::Document`] plus a
//! list of [`Diagnostic`]s. Parse errors do not abort — we recover and
//! keep going so the LSP gets a usable tree even mid-edit.

mod cook;
mod cursor;
mod parser;

pub use cook::{cook_call_content, cook_document, cook_run, CookedBlock, CookedDocument};

use stem_core::{Diagnostic, Document};

#[derive(Clone, Debug, Default)]
pub struct ParseResult {
    pub document: Document,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse a Stem source string into a [`Document`] plus diagnostics.
pub fn parse(src: &str) -> ParseResult {
    parser::parse(src)
}
