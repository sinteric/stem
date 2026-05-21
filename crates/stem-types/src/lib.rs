//! Function and property registry, plus a validator that walks a
//! [`stem_core::ast::Document`] and emits diagnostics for unknown
//! elements, illegal placement, or bad property values.
//!
//! Hand-keyed today; the schema declarations in `docs/schema.md` are
//! the human-facing source of truth and will eventually be machine-
//! extracted into this registry.

pub mod element;
pub mod elements;
pub mod schema;
pub mod validator;

pub use element::{DocTypeRef, ElementDef, ValidateFn};
pub use schema::{
    default_registry, BodyKind, Category, DocumentType, ElementSchema, PropertyDef, Registry,
    ValueKind,
};
pub use validator::validate;
