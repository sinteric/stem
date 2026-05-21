//! `p` — paragraph.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const P: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "p",
        categories: &[Category::BlockLeaf],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Text],
        parents: &["any-block-container"],
        children: &[],
        properties: &[PropertyDef {
            name: "align",
            kind: ValueKind::Enum(&["left", "center", "right", "justify"]),
            required: false,
            doc: "Horizontal alignment",
        }],
        doc: "Paragraph",
    },
    validate: None,
};
