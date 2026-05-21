//! `slide` — a single presentation slide.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const SLIDE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "slide",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Presentation],
        bodies: &[BodyKind::Children],
        parents: &["root"],
        children: &["any-block"],
        properties: &[
            PropertyDef { name: "id", kind: ValueKind::String, required: false, doc: "Slide identifier" },
            PropertyDef { name: "layout", kind: ValueKind::String, required: false, doc: "Layout name" },
            PropertyDef { name: "background", kind: ValueKind::Color, required: false, doc: "Slide background" },
        ],
        doc: "Single slide",
    },
    validate: None,
};
