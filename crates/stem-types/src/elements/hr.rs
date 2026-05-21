//! `hr` — horizontal rule.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema};

pub const HR: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "hr",
        categories: &[Category::BlockMarker],
        doc_types: &[DocumentType::Document],
        bodies: &[BodyKind::None],
        parents: &["any-block-container"],
        children: &[],
        properties: &[],
        doc: "Horizontal rule",
    },
    validate: None,
};
