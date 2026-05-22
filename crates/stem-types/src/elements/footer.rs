//! `footer` — page footer block. Doc-level only.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const FOOTER: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "footer",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Document],
        bodies: &[BodyKind::Children],
        parents: &["root"],
        children: &["any-block"],
        properties: &[PropertyDef {
            name: "scope",
            kind: ValueKind::Enum(&["default", "first", "even"]),
            required: false,
            doc: "Which pages this footer applies to (default = all unless overridden)",
        }],
        doc: "Page footer content for paged output",
    },
    validate: None,
};
