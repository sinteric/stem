//! `@formula(...)` — spreadsheet formula expression.
//!
//! The element is valid in any doc type but is most commonly placed as
//! the sole inline element inside a sheet `cell` body, where the cell's
//! computed value comes from evaluating it. In prose it evaluates with
//! an empty cell environment.
//!
//! Syntax errors surface at validate time via [`validate_formula`]: the
//! parser in [`crate::formula`] runs over the body text, and any
//! [`crate::formula::FormulaError`] is converted to a diagnostic with a
//! stable `formula.*` code.

use stem_core::ast::{Block, Body, TextPiece};
use stem_core::diagnostic::Diagnostic;

use crate::element::{DocTypeRef, ElementDef};
use crate::formula::parse_formula;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema};

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
    validate: Some(validate_formula),
};

fn validate_formula(block: &Block, _: &DocTypeRef) -> Vec<Diagnostic> {
    // Concatenate the body text pieces; ignore nested inlines (they
    // shouldn't appear in a formula body, but if they do the parser
    // will see the literal portion and complain).
    let mut src = String::new();
    if let Body::Text(pieces) = &block.body {
        for p in pieces {
            if let TextPiece::Literal { text, .. } = p {
                src.push_str(text);
            }
        }
    }
    if src.trim().is_empty() {
        return vec![];
    }
    match parse_formula(&src) {
        Ok(_) => vec![],
        Err(e) => vec![Diagnostic::error(
            e.code(),
            format!("formula: {e}"),
            block.span,
        )],
    }
}
