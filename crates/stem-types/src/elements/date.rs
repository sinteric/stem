//! `date` — semantic date span.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

const ALL_DT: &[DocumentType] = &[];

pub const DATE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "date",
        categories: &[Category::Inline, Category::BlockLeaf],
        doc_types: ALL_DT,
        bodies: &[BodyKind::Text],
        parents: &["any-text-body", "any-block-container"],
        children: &[],
        properties: &[PropertyDef {
            name: "format",
            kind: ValueKind::String,
            required: false,
            doc: "Display format hint",
        }],
        doc: "A semantic date span",
    },
    validate: None,
};
