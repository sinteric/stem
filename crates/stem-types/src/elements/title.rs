//! `title` — slide title.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const TITLE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "title",
        categories: &[Category::BlockLeaf],
        doc_types: &[DocumentType::Presentation],
        bodies: &[BodyKind::Text],
        parents: &["slide"],
        children: &[],
        properties: &[PropertyDef {
            name: "size",
            kind: ValueKind::Length,
            required: false,
            doc: "Override default title size",
        }],
        doc: "Slide title",
    },
    validate: None,
};
