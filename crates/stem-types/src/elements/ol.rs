//! `ol` — ordered list.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const OL: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "ol",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Children],
        parents: &["any-block-container", "li"],
        children: &["li"],
        properties: &[
            PropertyDef { name: "style", kind: ValueKind::Style, required: false, doc: "Marker style" },
            PropertyDef { name: "start", kind: ValueKind::Integer, required: false, doc: "Starting position" },
        ],
        doc: "Ordered list",
    },
    validate: None,
};
