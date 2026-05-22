//! Import external document formats into the Stem AST.
//!
//! One module per format. Each module gates itself behind a Cargo
//! feature with the same name, so consumers compile only the formats
//! they need:
//!
//! ```toml
//! stem-imports = { version = "0.1", features = ["markdown", "docx"] }
//! ```
//!
//! Each module implements [`stem_core::Importer`]. The trait is the
//! contract: take a representation of an external document, produce a
//! Stem [`Document`](stem_core::ast::Document).
//!
//! Stem's own source language is parsed by `stem-parser`, not by any
//! `Importer` impl here — the language is the canonical input, not one
//! format among many.

#[cfg(feature = "markdown")]
pub mod markdown;

#[cfg(feature = "markdown")]
pub use markdown::MarkdownImporter;

pub use stem_core::Importer;
