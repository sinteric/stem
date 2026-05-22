//! `header` — page header block. Doc-level only.
//!
//! Contains the blocks that should appear at the top of every page in
//! paged output (docx, PDF). Treated as metadata: only allowed as a
//! direct child of root, and exporters that don't paginate (HTML,
//! markdown) may render it as a header section or skip it.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const HEADER: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "header",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Document],
        bodies: &[BodyKind::Children],
        parents: &["root"],
        children: &["any-block"],
        properties: &[PropertyDef {
            name: "scope",
            kind: ValueKind::Enum(&["default", "first", "even"]),
            required: false,
            doc: "Which pages this header applies to (default = all unless overridden)",
        }],
        doc: "Page header content for paged output",
    },
    validate: None,
};
