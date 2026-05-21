//! `section` — top-level structural division of a document.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const SECTION: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "section",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Document],
        bodies: &[BodyKind::Children, BodyKind::None],
        parents: &["root", "section"],
        children: &["any-block"],
        properties: &[
            PropertyDef { name: "id", kind: ValueKind::String, required: false, doc: "Section identifier" },
            PropertyDef { name: "title", kind: ValueKind::String, required: false, doc: "Display title override" },
        ],
        doc: "Top-level structural division of a document",
    },
    validate: None,
};
