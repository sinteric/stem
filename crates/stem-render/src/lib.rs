//! Render Stem documents to output formats.
//!
//! The architecture is a small trait, [`Renderer`], with one concrete
//! implementation per output format. Renderers consume a cooked
//! [`stem_core::ast::Document`] and a [`stem_core::theme::Theme`].
//! They never see source text or the schema registry, which forces
//! them to think in terms of *layout intent* rather than source syntax.

pub mod docx;
pub mod html;
pub mod math;
pub mod pdf;

pub use docx::DocxRenderer;
pub use html::HtmlRenderer;
pub use pdf::PdfRenderer;

use stem_core::ast::Document;
use stem_core::theme::Theme;

/// The renderer contract. `Output` is whatever the format produces —
/// `String` for HTML, `Vec<u8>` for docx/pdf.
pub trait Renderer {
    type Output;
    type Error: std::error::Error + Send + Sync + 'static;
    fn render(&self, doc: &Document, theme: &Theme) -> Result<Self::Output, Self::Error>;
}
