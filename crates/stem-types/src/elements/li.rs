//! `li` — list item.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const LI: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "li",
        categories: &[Category::BlockLeaf, Category::BlockContainer],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Text, BodyKind::Children],
        parents: &["ol", "ul"],
        children: &["any-block"],
        properties: &[PropertyDef {
            name: "at",
            kind: ValueKind::Integer,
            required: false,
            doc: "Override this item's position",
        }],
        doc: "List item",
    },
    validate: None,
};
