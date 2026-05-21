//! `layout` — multi-column layout container.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const LAYOUT: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "layout",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Children],
        parents: &["any-block-container"],
        children: &["col"],
        properties: &[
            PropertyDef {
                name: "kind",
                kind: ValueKind::Enum(&["two-column", "three-column", "sidebar"]),
                required: true,
                doc: "Layout variant",
            },
            PropertyDef { name: "gap", kind: ValueKind::Length, required: false, doc: "Inter-column gap" },
        ],
        doc: "Multi-column layout container",
    },
    validate: None,
};
