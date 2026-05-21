//! `text` — styled inline text span.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

const ALL_DT: &[DocumentType] = &[];

pub const TEXT: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "text",
        categories: &[Category::Inline, Category::BlockLeaf],
        doc_types: ALL_DT,
        bodies: &[BodyKind::Text],
        parents: &["any-text-body", "any-block-container"],
        children: &[],
        properties: &[
            PropertyDef { name: "color", kind: ValueKind::Color, required: false, doc: "Foreground color" },
            PropertyDef { name: "bg", kind: ValueKind::Color, required: false, doc: "Background color" },
            PropertyDef {
                name: "weight",
                kind: ValueKind::Enum(&["light", "regular", "bold"]),
                required: false,
                doc: "Font weight",
            },
            PropertyDef {
                name: "style",
                kind: ValueKind::Enum(&["italic", "oblique", "normal"]),
                required: false,
                doc: "Font slant",
            },
            PropertyDef {
                name: "decoration",
                kind: ValueKind::Enum(&["none", "underline", "strike"]),
                required: false,
                doc: "Text decoration",
            },
        ],
        doc: "Styled inline text span",
    },
    validate: None,
};
