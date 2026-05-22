//! Importer and Exporter traits.
//!
//! Stem treats other document formats as a star pattern around the
//! `Document` AST. Every external format has an [`Importer`] (format
//! → `Document`) and/or an [`Exporter`] (`Document` → format).
//! Implementations live in the `stem-imports` and `stem-exports`
//! sibling crates, organized one module per format.
//!
//! Stem's own source language uses the dedicated `stem-parser` crate
//! rather than implementing `Importer` — the language is the canonical
//! input, not one format among many.

use crate::ast::Document;
use crate::theme::Theme;

/// Convert an external document representation into a Stem AST.
///
/// Implementations live in `stem-imports::<format>`. The associated
/// `Input` type is typically `&str` for textual formats and `&[u8]`
/// for binary formats; implementations are free to choose.
pub trait Importer {
    type Input;
    type Error: std::error::Error + Send + Sync + 'static;
    fn import(&self, input: Self::Input) -> Result<Document, Self::Error>;
}

/// Convert a Stem AST into an external document representation.
///
/// Implementations live in `stem-exports::<format>`. The associated
/// `Output` type is `String` for text/HTML targets and `Vec<u8>` for
/// binary targets (docx, xlsx, pdf, …).
pub trait Exporter {
    type Output;
    type Error: std::error::Error + Send + Sync + 'static;
    fn export(&self, doc: &Document, theme: &Theme) -> Result<Self::Output, Self::Error>;
}
