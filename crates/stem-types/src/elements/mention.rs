//! `mention` — reference to a person, team, or entity.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

const ALL_DT: &[DocumentType] = &[];

pub const MENTION: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "mention",
        categories: &[Category::Inline],
        doc_types: ALL_DT,
        bodies: &[BodyKind::Text],
        parents: &["any-text-body"],
        children: &[],
        properties: &[PropertyDef {
            name: "handle",
            kind: ValueKind::String,
            required: false,
            doc: "Backing handle/identifier",
        }],
        doc: "Reference to a person, team, or entity",
    },
    validate: None,
};
