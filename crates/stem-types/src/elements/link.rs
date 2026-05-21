//! `@link[to:..., title:...]` — hyperlink.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

const ALL_DOC_TYPES: &[DocumentType] = &[];

pub const LINK: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "link",
        categories: &[Category::Inline],
        doc_types: ALL_DOC_TYPES,
        bodies: &[BodyKind::Text],
        parents: &["any-text-body"],
        children: &[],
        properties: &[
            PropertyDef {
                name: "to",
                kind: ValueKind::String,
                required: true,
                doc: "Target URL or cross-ref",
            },
            PropertyDef {
                name: "title",
                kind: ValueKind::String,
                required: false,
                doc: "Tooltip text",
            },
        ],
        doc: "Hyperlink",
    },
    validate: None,
};
