//! Render Stem documents to output formats.
//!
//! The architecture is a small trait, [`Renderer`], with one concrete
//! implementation per output format. Renderers consume a cooked document
//! (from `stem_parser::cook_document`) and a [`Theme`]; they never see
//! source text or the registry, which forces them to think in terms of
//! *layout intent* rather than source syntax.

use stem_core::ast::{Document, FunctionCall};
use stem_core::theme::Theme;

pub mod html;
pub mod docx;
pub mod pdf;

pub use html::HtmlRenderer;
pub use docx::DocxRenderer;
pub use pdf::PdfRenderer;

/// The renderer contract. `Output` is whatever the format produces —
/// `String` for HTML, `Vec<u8>` for docx/pdf.
pub trait Renderer {
    type Output;
    type Error: std::error::Error + Send + Sync + 'static;
    fn render(&self, doc: &Document, theme: &Theme) -> Result<Self::Output, Self::Error>;
}

/// Helper for renderers that want to recognise standard structural calls
/// (`section`, `layout`, `col`, `table`, etc.) when walking the AST.
/// Each renderer is free to ignore unknown calls or render a fallback —
/// this matches the "graceful degradation" expectation.
pub mod intent {
    use super::FunctionCall;

    pub fn is_section(c: &FunctionCall) -> bool {
        c.name == "section"
    }
    pub fn is_layout(c: &FunctionCall) -> bool {
        c.name == "layout"
    }
    pub fn is_col(c: &FunctionCall) -> bool {
        c.name == "col"
    }
    pub fn is_table(c: &FunctionCall) -> bool {
        c.name == "table"
    }
    pub fn is_row(c: &FunctionCall) -> bool {
        c.name == "row"
    }
    pub fn is_cell(c: &FunctionCall) -> bool {
        c.name == "cell"
    }
    pub fn is_text_span(c: &FunctionCall) -> bool {
        c.name == "text"
    }
    pub fn is_footnote(c: &FunctionCall) -> bool {
        c.name == "footnote"
    }
    pub fn is_note(c: &FunctionCall) -> bool {
        c.name == "note"
    }
    pub fn is_date(c: &FunctionCall) -> bool {
        c.name == "date"
    }
    pub fn is_pagebreak(c: &FunctionCall) -> bool {
        c.name == "pagebreak"
    }
    pub fn is_toc(c: &FunctionCall) -> bool {
        c.name == "toc"
    }
}
