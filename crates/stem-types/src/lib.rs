//! Function and property registry, plus a validator that walks a
//! [`stem_core::ast::Document`] and emits diagnostics for unknown
//! elements, illegal placement, or bad property values.
//!
//! Hand-keyed today; the schema declarations in `docs/schema.md` are
//! the human-facing source of truth and will eventually be machine-
//! extracted into this registry.

pub mod element;
pub mod elements;
pub mod formula;
pub mod schema;
pub mod validator;

pub use element::{DocTypeRef, ElementDef, ValidateFn};
pub use schema::{
    default_registry, BodyKind, Category, DocumentType, ElementSchema, PropertyDef, Registry,
    ValueKind,
};
pub use validator::validate;

/// Highest heading level the document type defines (`h1`..`h<MAX>`).
/// Element files `crates/stem-types/src/elements/heading.rs` declare
/// exactly this many heading constants; the docx exporter's TOC
/// instruction and the `Heading1..N` style registration both pin to
/// this number so any future widening to e.g. `h7` is a single-place
/// change.
pub const MAX_HEADING_LEVEL: usize = 6;
