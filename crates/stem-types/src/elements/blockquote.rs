//! `blockquote` — multi-line quotation block.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const BLOCKQUOTE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "blockquote",
        categories: &[Category::BlockLeaf],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Text],
        parents: &["any-block-container"],
        children: &[],
        properties: &[PropertyDef {
            name: "cite",
            kind: ValueKind::String,
            required: false,
            doc: "Source citation",
        }],
        doc: "Multi-line quotation block",
    },
    validate: None,
};
