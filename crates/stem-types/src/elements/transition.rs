//! `transition` — slide-to-slide animation hint.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const TRANSITION: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "transition",
        categories: &[Category::BlockMarker],
        doc_types: &[DocumentType::Presentation],
        bodies: &[BodyKind::None],
        parents: &["slide"],
        children: &[],
        properties: &[
            PropertyDef {
                name: "kind",
                kind: ValueKind::Enum(&["none", "fade", "slide-left", "slide-right", "zoom"]),
                required: true,
                doc: "Transition type",
            },
            PropertyDef { name: "duration", kind: ValueKind::Length, required: false, doc: "Duration" },
        ],
        doc: "Slide transition",
    },
    validate: None,
};
