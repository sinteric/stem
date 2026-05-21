//! `math` — inline or block math expression.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

const ALL_DT: &[DocumentType] = &[];

pub const MATH: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "math",
        categories: &[Category::Inline, Category::BlockLeaf],
        doc_types: ALL_DT,
        bodies: &[BodyKind::Text],
        parents: &["any-text-body", "any-block-container"],
        children: &[],
        properties: &[
            PropertyDef {
                name: "notation",
                kind: ValueKind::Enum(&["latex", "asciimath", "mathml"]),
                required: false,
                doc: "Notation system (default: latex)",
            },
            PropertyDef {
                name: "display",
                kind: ValueKind::Enum(&["inline", "block"]),
                required: false,
                doc: "Render style",
            },
        ],
        doc: "Inline or block math expression",
    },
    validate: None,
};
