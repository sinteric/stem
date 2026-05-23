//! Stem AST → .docx via direct OOXML emission.
//!
//! Emits each OOXML part as a string and packages them into the
//! `.docx` ZIP directly. Children of every container element are
//! written in the canonical schema order on the first pass — no
//! post-process repair step.
//!
//! Scope matches the WordprocessingML subset the academic-paper
//! template uses; the visual target is
//! `references/docx/paper_boringcrypto_security_policy.docx`.
//!
//! Layout:
//! - `parts/` — top-level OOXML parts (document, styles, numbering,
//!   theme, settings, header/footer, footnotes, doc props, …).
//! - `emit/` — per-shape body emitters (paragraph, run, table,
//!   drawing, hyperlink, field, toc) plus the shared `EmitCtx`.
//! - `package.rs` — ZIP packaging.
//! - `xml.rs` — small string-based XML builder with namespaces.

use std::path::{Path, PathBuf};

use stem_core::ast::Document;
use stem_core::theme::Theme;
use stem_core::Exporter;
use thiserror::Error;

// Some helpers in the part/emit subtree are reserved for future
// shapes (e.g. anchor positioning variants the academic-paper output
// doesn't currently exercise). Tolerate the unused warnings until
// those callers land.
#[allow(dead_code)]
mod emit;
#[allow(dead_code)]
mod package;
#[allow(dead_code)]
mod parts;
#[allow(dead_code)]
mod xml;

#[derive(Default)]
pub struct DocxExporter {
    /// Directory used to resolve relative `image[src:..]` paths. When
    /// unset, relative paths resolve against the process's current
    /// working directory.
    image_base: Option<PathBuf>,
}

impl DocxExporter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve relative image paths against `base`. Absolute paths
    /// are still used verbatim.
    pub fn with_image_base(mut self, base: impl AsRef<Path>) -> Self {
        self.image_base = Some(base.as_ref().to_path_buf());
        self
    }
}

#[derive(Debug, Error)]
pub enum DocxError {
    #[error("docx pack: {0}")]
    Pack(String),
    #[error("docx image: failed to read {path}: {source}")]
    ImageRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl Exporter for DocxExporter {
    type Output = Vec<u8>;
    type Error = DocxError;
    fn export(&self, doc: &Document, _theme: &Theme) -> Result<Vec<u8>, DocxError> {
        let cooked = stem_parser::cook_document(doc);
        parts::package_doc(&cooked, self.image_base.as_deref())
    }
}
