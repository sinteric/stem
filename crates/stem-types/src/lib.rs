//! Function and property registry, plus a validator that walks an AST
//! and emits diagnostics for unknown functions, illegal placement, or
//! bad property values.
//!
//! The registry is per-document-type (`document`, `presentation`,
//! `sheet`) and lives behind a small builder so applications can register
//! their own.

use std::collections::BTreeMap;

use stem_core::ast::*;
use stem_core::diagnostic::Diagnostic;
use stem_core::span::Span;

pub mod schema;
pub mod validator;

pub use schema::{
    ArgArity, DocumentType, FunctionSchema, PropertySchema, Registry, ValueKind,
};
pub use validator::validate;

/// Convenience: build the registry of the bundled document types
/// (`document`, `presentation`, `sheet`).
pub fn default_registry() -> Registry {
    schema::default_registry()
}

/// Re-export for callers that want a one-call entry point.
pub fn validate_document(doc: &Document) -> Vec<Diagnostic> {
    validate(doc, &default_registry())
}

// trivial smoke test the consumer-facing surface compiles
#[doc(hidden)]
pub fn _smoke(_: BTreeMap<String, FunctionSchema>) -> Vec<Diagnostic> {
    Vec::new()
}

// Ensure types we re-export are reachable.
const _: fn() = || {
    let _ = Span::default();
};
