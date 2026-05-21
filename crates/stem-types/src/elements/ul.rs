//! `ul` — unordered list.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const UL: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "ul",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Children],
        parents: &["any-block-container", "li"],
        children: &["li"],
        properties: &[PropertyDef {
            name: "style",
            kind: ValueKind::Enum(&["disc", "circle", "square", "dash", "none"]),
            required: false,
            doc: "Bullet style",
        }],
        doc: "Unordered list",
    },
    validate: None,
};
