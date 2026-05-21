//! `code` — inline or block monospace code.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

const ALL_DT: &[DocumentType] = &[];

pub const CODE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "code",
        categories: &[Category::Inline, Category::BlockLeaf],
        doc_types: ALL_DT,
        bodies: &[BodyKind::Text],
        parents: &["any-text-body", "any-block-container"],
        children: &[],
        properties: &[
            PropertyDef { name: "lang", kind: ValueKind::String, required: false, doc: "Source language" },
            PropertyDef { name: "numbered", kind: ValueKind::Bool, required: false, doc: "Show line numbers" },
        ],
        doc: "Inline or block monospace code",
    },
    validate: None,
};
