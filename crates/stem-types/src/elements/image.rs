//! `image` — image with required alt text and optional caption.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const IMAGE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "image",
        categories: &[Category::BlockMarker],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::None],
        parents: &["any-block-container"],
        children: &[],
        properties: &[
            PropertyDef { name: "src", kind: ValueKind::String, required: true, doc: "Image path or URL" },
            PropertyDef { name: "alt", kind: ValueKind::String, required: true, doc: "Alt text for accessibility" },
            PropertyDef { name: "w", kind: ValueKind::Length, required: false, doc: "Width" },
            PropertyDef { name: "h", kind: ValueKind::Length, required: false, doc: "Height" },
            PropertyDef { name: "caption", kind: ValueKind::String, required: false, doc: "Visible caption" },
            PropertyDef {
                name: "float",
                kind: ValueKind::Enum(&["inline", "anchor", "behind"]),
                required: false,
                doc: "Image positioning: inline (in text flow, default), anchor (floats with text wrap), behind (anchored behind text)",
            },
        ],
        doc: "Image with required alt and optional caption",
    },
    validate: None,
};
