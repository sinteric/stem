//! `sheet` — top-level spreadsheet container.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const SHEET: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "sheet",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Sheet],
        bodies: &[BodyKind::Children],
        parents: &["root"],
        children: &["cell", "col", "row", "fill", "source", "named", "format", "chart"],
        properties: &[
            PropertyDef { name: "id", kind: ValueKind::String, required: false, doc: "Sheet id" },
            PropertyDef { name: "name", kind: ValueKind::String, required: false, doc: "Display name" },
            PropertyDef { name: "freeze", kind: ValueKind::Address, required: false, doc: "Freeze pane address" },
        ],
        doc: "Sheet tab",
    },
    validate: None,
};
