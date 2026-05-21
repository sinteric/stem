//! Per-element definitions.
//!
//! An [`ElementDef`] bundles everything `stem-types` needs to know about
//! a single element name: its schema, and (optionally) a semantic
//! validator that runs after schema-level checks.
//!
//! Element definitions live alongside their owning concern. Vocabulary
//! definitions are in [`crate::elements`]; rendering is per-backend in
//! `stem-render`.
//!
//! Rationale: previously every element was scattered across `schema.rs`
//! (one big `BUILTINS` array) and per-renderer match arms. Per-element
//! modules collect each element's vocabulary in one place. Renderers
//! consult their own per-backend dispatch tables that mirror this layout.

use stem_core::ast::Block;
use stem_core::diagnostic::Diagnostic;

use crate::schema::{DocumentType, ElementSchema};

/// A reference to the active document type, passed to validators.
///
/// A `&DocTypeRef` (rather than a bare [`DocumentType`]) leaves room to
/// attach extra context later — e.g., per-doc-type configuration — without
/// breaking the validator signature.
#[derive(Clone, Copy, Debug)]
pub struct DocTypeRef<'a> {
    pub doc_type: DocumentType,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> DocTypeRef<'a> {
    pub fn new(doc_type: DocumentType) -> Self {
        Self {
            doc_type,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Function-pointer signature for semantic validation hooks.
///
/// Function pointers (not closures) keep `ElementDef` `const`-friendly:
/// every element definition is a static value, not a runtime allocation.
pub type ValidateFn = fn(&Block, &DocTypeRef) -> Vec<Diagnostic>;

/// Complete definition of a single element from the vocabulary layer's
/// point of view. Renderer-specific code lives in `stem-render`.
#[derive(Clone, Debug)]
pub struct ElementDef {
    pub schema: ElementSchema,
    pub validate: Option<ValidateFn>,
}
