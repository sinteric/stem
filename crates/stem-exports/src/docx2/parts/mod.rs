//! OOXML parts. Each submodule emits one part (or family of parts)
//! as a `String` ready to be added to the ZIP package.

use super::package::Package;
use super::DocxV2Error;

pub mod content_types;
pub mod document;
pub mod rels;

/// Minimal valid empty .docx: just the four parts Word requires to
/// open the file — `[Content_Types].xml`, the root rels, the
/// document part, and the document's rels.
///
/// Task 1 scaffold, exercised end-to-end through the task-2
/// builder/packager. Subsequent tasks add styles, numbering, theme,
/// header/footer, real body content, etc.
pub fn minimal_empty_doc() -> Result<Vec<u8>, DocxV2Error> {
    let mut pkg = Package::new();
    pkg.add_text("[Content_Types].xml", content_types::minimal());
    pkg.add_text("_rels/.rels", rels::root());
    pkg.add_text("word/_rels/document.xml.rels", rels::document_minimal());
    pkg.add_text("word/document.xml", document::minimal());
    pkg.finish()
}
