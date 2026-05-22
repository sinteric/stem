//! Export Stem documents to external formats.
//!
//! One module per format. Each module gates itself behind a Cargo
//! feature with the same name, so consumers compile only the formats
//! they need:
//!
//! ```toml
//! stem-exports = { version = "0.1", features = ["html", "docx"] }
//! ```
//!
//! Each module implements [`stem_core::Exporter`]. The trait is the
//! contract: take a cooked [`stem_core::ast::Document`] and a
//! [`stem_core::theme::Theme`], produce an output (text for HTML and
//! Markdown, bytes for docx/xlsx/pdf/etc.).

#[cfg(feature = "docx")]
pub mod docx;
#[cfg(feature = "docx2")]
pub mod docx2;
#[cfg(feature = "html")]
pub mod html;
#[cfg(feature = "html")]
pub mod math;
#[cfg(feature = "markdown")]
pub mod markdown;
#[cfg(feature = "pdf")]
pub mod pdf;
#[cfg(feature = "xlsx")]
pub mod xlsx;

#[cfg(feature = "docx")]
pub use docx::{DocxError, DocxExporter};
#[cfg(feature = "docx2")]
pub use docx2::{DocxV2Error, DocxV2Exporter};
#[cfg(feature = "html")]
pub use html::HtmlExporter;
#[cfg(feature = "markdown")]
pub use markdown::MarkdownExporter;
#[cfg(feature = "pdf")]
pub use pdf::PdfExporter;
#[cfg(feature = "xlsx")]
pub use xlsx::XlsxExporter;

// Re-export the trait so consumers don't need to depend on stem-core
// just to call exports.
pub use stem_core::Exporter;
