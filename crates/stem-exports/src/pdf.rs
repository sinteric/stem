//! Stub PDF renderer. The interface is fully specified; the
//! implementation is intentionally a stub.
//!
//! ## Implementation contract for a future replacement
//!
//! Two reasonable paths:
//!
//! - **HTML → headless Chromium / wkhtmltopdf / weasyprint**: render the
//!   document with `HtmlExporter { full_document: true, .. }` and feed
//!   the result into your favourite HTML-to-PDF converter. Cheapest to
//!   ship and gets all of CSS for free.
//!
//! - **Native typst pipeline**: emit `.typ` source from the cooked
//!   document tree, then either:
//!     * shell out to the `typst` binary, or
//!     * link `typst` as a library (since 0.13) and compile in-process.
//!   This path gives true paged layout and is the recommended target
//!   for production document delivery.
//!
//! The mapping mostly mirrors the HTML renderer:
//! - `section` → typst `#heading` plus body
//! - `layout` → `#grid(columns: ..., ...)`
//! - `table` → typst `#table`
//! - inline `text` → `#text(fill: ..., weight: ...)`
//!
//! Theme colors map to `rgb("#rrggbb")` literals.

use stem_core::ast::Document;
use stem_core::theme::Theme;
use stem_core::Exporter;

#[derive(Default)]
pub struct PdfExporter;

impl PdfExporter {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("pdf exporter not yet implemented — see crates/stem-exports/src/pdf.rs for contract")]
    NotImplemented,
}

impl Exporter for PdfExporter {
    type Output = Vec<u8>;
    type Error = PdfError;

    fn export(&self, _doc: &Document, _theme: &Theme) -> Result<Vec<u8>, Self::Error> {
        Err(PdfError::NotImplemented)
    }
}
