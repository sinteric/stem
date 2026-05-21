//! Stem parser. Turns source text into a [`stem_core::ast::Document`]
//! plus a list of diagnostics. Parse errors do not abort — we recover
//! and keep going so the LSP gets a usable tree even mid-edit.

mod cook;
mod csv;
mod cursor;
mod parser;

pub use cook::{cook_document, cook_document_with, CookOptions, CookResult, FileLoader};
pub use csv::{parse_csv, CsvOptions, CsvTable};
pub use parser::{parse, ParseResult};
