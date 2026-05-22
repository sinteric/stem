//! Stem AST → .docx via direct OOXML emission.
//!
//! Successor to [`crate::docx`] (which goes through `docx-rs` and
//! carries a stack of repair passes to fix the schema-order bugs that
//! library produces). This module emits the OOXML parts as XML
//! strings and packages them into a ZIP directly, so the output is
//! correct on the first pass — no rewrite step.
//!
//! Scope is the same WordprocessingML subset the academic-paper
//! template uses; the goal is a 1:1 visual match with
//! `references/docx/paper_boringcrypto_security_policy.docx`.
//!
//! This module is feature-gated behind `docx2` while the migration is
//! in progress. The current `docx` module stays the default until the
//! migration switch (task 15 in the migration plan).

use std::path::{Path, PathBuf};

use stem_core::ast::Document;
use stem_core::theme::Theme;
use stem_core::Exporter;
use thiserror::Error;

// The builder + part-emitter API surface is wider than the minimal
// scaffold consumes. Each subsequent task wires up another slice
// (styles, numbering, paragraphs, runs, tables, ...). Until they
// land, suppress the dead-code warnings so the build stays clean.
#[allow(dead_code)]
mod package;
#[allow(dead_code)]
mod parts;
#[allow(dead_code)]
mod xml;

#[derive(Default)]
pub struct DocxV2Exporter {
    /// Directory used to resolve relative `image[src:..]` paths. When
    /// unset, relative paths resolve against the process's current
    /// working directory.
    image_base: Option<PathBuf>,
}

impl DocxV2Exporter {
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
pub enum DocxV2Error {
    #[error("docx2 pack: {0}")]
    Pack(String),
    #[error("docx2 image: failed to read {path}: {source}")]
    ImageRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl Exporter for DocxV2Exporter {
    type Output = Vec<u8>;
    type Error = DocxV2Error;
    fn export(&self, doc: &Document, _theme: &Theme) -> Result<Vec<u8>, DocxV2Error> {
        // Task 1+2 scaffold: emit a minimal valid empty docx. The
        // cooked AST is not consumed yet — body emission is task 6.
        let _cooked = stem_parser::cook_document(doc);
        parts::minimal_empty_doc()
    }
}
