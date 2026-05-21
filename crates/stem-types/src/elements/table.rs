//! `table` — document-style table.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const TABLE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "table",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Children],
        parents: &["any-block-container"],
        children: &["row"],
        properties: &[
            PropertyDef {
                name: "border",
                kind: ValueKind::Enum(&["none", "outer", "all"]),
                required: false,
                doc: "Border policy",
            },
            PropertyDef { name: "stripe", kind: ValueKind::Bool, required: false, doc: "Alternate row backgrounds" },
            PropertyDef { name: "caption", kind: ValueKind::String, required: false, doc: "Table caption" },
        ],
        doc: "Document-style table",
    },
    validate: None,
};
