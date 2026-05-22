//! OOXML parts. Each submodule emits one part (or family of parts)
//! as a byte buffer ready to be added to the ZIP package.

use super::package::Part;

mod content_types;
mod document;
mod rels;

/// Minimal valid empty .docx: just the four parts Word requires to
/// open the file — `[Content_Types].xml`, the root rels, the
/// document part, and the document's rels. No styles, no body
/// content beyond a single empty paragraph.
///
/// Task 1 scaffold. Subsequent tasks add styles, numbering, theme,
/// header/footer, document body, etc.
pub fn minimal_empty_doc() -> Vec<Part> {
    vec![
        Part::from_str("[Content_Types].xml", content_types::minimal()),
        Part::from_str("_rels/.rels", rels::root()),
        Part::from_str("word/_rels/document.xml.rels", rels::document_minimal()),
        Part::from_str("word/document.xml", document::minimal()),
    ]
}
