//! `pagebreak` — forced page break in paged output.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema};

pub const PAGEBREAK: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "pagebreak",
        categories: &[Category::BlockMarker],
        doc_types: &[DocumentType::Document],
        bodies: &[BodyKind::None],
        parents: &["any-block-container"],
        children: &[],
        properties: &[],
        doc: "Force a page break in paged output",
    },
    validate: None,
};
