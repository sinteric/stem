//! `note` — callout note (info / warning / tip / caution).

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const NOTE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "note",
        categories: &[Category::BlockLeaf],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Text],
        parents: &["any-block-container"],
        children: &[],
        properties: &[PropertyDef {
            name: "kind",
            kind: ValueKind::Enum(&["info", "warning", "tip", "caution"]),
            required: false,
            doc: "Visual variant",
        }],
        doc: "Callout note",
    },
    validate: None,
};
