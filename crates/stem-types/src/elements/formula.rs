//! `@formula(...)` — spreadsheet formula expression.
//!
//! The element is valid in any doc type but is most commonly placed as
//! the sole inline element inside a sheet `cell` body, where the cell's
//! computed value comes from evaluating it. In prose it evaluates with
//! an empty cell environment.
//!
//! The formula parser and evaluator live in `stem-render::formula` for
//! now. Wiring them into validate-time syntax checking requires either
//! moving the parser to a crate `stem-types` can depend on, or making
//! `stem-types` depend on `stem-render` (currently a circular concern).
//! Deferred — this migration is structural only.

use crate::element::ElementDef;
use crate::schema::{
    BodyKind, Category, DocumentType, ElementSchema,
};

const ALL_DOC_TYPES: &[DocumentType] = &[];

pub const FORMULA: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "formula",
        categories: &[Category::Inline, Category::BlockLeaf],
        doc_types: ALL_DOC_TYPES,
        bodies: &[BodyKind::Text],
        parents: &["any-text-body", "any-block-container"],
        children: &[],
        properties: &[],
        doc: "Spreadsheet formula expression. The body is the formula text (no leading `=`). Inside a `cell` body it becomes the cell's computed value; in prose, it evaluates with an empty cell env.",
    },
    validate: None,
};
