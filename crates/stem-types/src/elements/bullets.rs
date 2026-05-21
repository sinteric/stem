//! `bullets` — slide bullet list (presentation analog of `ul`).

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const BULLETS: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "bullets",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Presentation],
        bodies: &[BodyKind::Children],
        parents: &["slide", "col"],
        children: &["item"],
        properties: &[PropertyDef {
            name: "style",
            kind: ValueKind::Enum(&["disc", "dash", "arrow", "number"]),
            required: false,
            doc: "Marker style",
        }],
        doc: "Slide bullet list",
    },
    validate: None,
};
