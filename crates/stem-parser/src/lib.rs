//! Stem parser. Turns source text into a [`stem_core::Document`] plus a
//! list of [`Diagnostic`]s. Parse errors do not abort — we recover and
//! keep going so the LSP gets a usable tree even mid-edit.

mod cook;
mod cook_v2;
mod csv;
mod cursor;
pub mod formula;
mod parser;
mod parser_v2;

pub use cook::{cook_call_content, cook_document, cook_run, CookedBlock, CookedDocument};
pub use cook_v2::{cook_document_v2, cook_document_v2_with, CookOptions, CookResult, FileLoader};
pub use csv::{parse_csv, CsvOptions, CsvTable};
pub use parser_v2::{parse as parse_v2, ParseResultV2};

use stem_core::{Diagnostic, Document};

#[derive(Clone, Debug, Default)]
pub struct ParseResult {
    pub document: Document,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse a Stem source string into a v1 [`Document`] plus diagnostics.
pub fn parse(src: &str) -> ParseResult {
    parser::parse(src)
}
