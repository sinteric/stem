//! `footnote` — inline footnote reference.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

const ALL_DT: &[DocumentType] = &[];

pub const FOOTNOTE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "footnote",
        categories: &[Category::Inline],
        doc_types: ALL_DT,
        bodies: &[BodyKind::Text],
        parents: &["any-text-body"],
        children: &[],
        properties: &[PropertyDef {
            name: "id",
            kind: ValueKind::String,
            required: false,
            doc: "Stable footnote id for cross-references",
        }],
        doc: "Inline footnote reference",
    },
    validate: None,
};
