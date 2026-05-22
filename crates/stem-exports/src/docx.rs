//! Stub `.docx` exporter. The interface is fully specified; the
//! implementation is intentionally a NotImplemented stub so the workspace
//! compiles and downstream tools can wire against the type without
//! pulling in heavy dependencies.
//!
//! ## Implementation contract for a future replacement
//!
//! 1. Build a `docx_rs::Docx` (or roll your own zip + XML) holding the
//!    word/document.xml, [Content_Types].xml, word/styles.xml,
//!    word/_rels/document.xml.rels, and `[Content_Types]`.
//! 2. Map theme colors → named styles in `word/styles.xml`; emit them
//!    once and reference by name from each run.
//! 3. Walk the cooked document (via `stem_parser::cook_document`):
//!    - `Block::Heading` → paragraph with `pStyle` = `Heading{level}`
//!    - `Block::Paragraph` → standard paragraph with runs
//!    - `Block::List` → numbered/bulleted list using a `numId`
//!    - `Block::Call` →
//!        * `section` → page break + heading
//!        * `layout`/`col` → tables with hidden borders (docx idiom)
//!        * `table` → real `w:tbl`
//!        * `pagebreak` → `<w:br w:type="page"/>`
//! 4. Inline calls (`text`, `footnote`, `date`) → runs with `rPr` for
//!    color/weight, footnote reference for `footnote`.
//!
//! The trait shape matches `HtmlExporter` so swapping in a real
//! implementation is a one-liner at the CLI.

use stem_core::ast::Document;
use stem_core::theme::Theme;
use stem_core::Exporter;

#[derive(Default)]
pub struct DocxExporter;

impl DocxExporter {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DocxError {
    #[error("docx exporter not yet implemented — see crates/stem-exports/src/docx.rs for contract")]
    NotImplemented,
}

impl Exporter for DocxExporter {
    type Output = Vec<u8>;
    type Error = DocxError;

    fn export(&self, _doc: &Document, _theme: &Theme) -> Result<Vec<u8>, Self::Error> {
        Err(DocxError::NotImplemented)
    }
}
