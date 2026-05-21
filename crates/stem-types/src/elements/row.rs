//! `row` — two distinct schemas:
//!
//! * Document table row (child of `table`).
//! * Sheet row-level properties (address-bearing, child of `sheet`).

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const ROW_DOC: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "row",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Children],
        parents: &["table"],
        children: &["cell"],
        properties: &[PropertyDef {
            name: "kind",
            kind: ValueKind::Enum(&["data", "header", "footer"]),
            required: false,
            doc: "Row role",
        }],
        doc: "Table row",
    },
    validate: None,
};

pub const ROW_SHEET: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "row",
        categories: &[Category::BlockMarker],
        doc_types: &[DocumentType::Sheet],
        bodies: &[BodyKind::None],
        parents: &["sheet"],
        children: &[],
        properties: &[
            PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Row number or range" },
            PropertyDef { name: "height", kind: ValueKind::Length, required: false, doc: "Row height" },
            PropertyDef {
                name: "weight",
                kind: ValueKind::Enum(&["light", "regular", "bold"]),
                required: false,
                doc: "Font weight",
            },
            PropertyDef { name: "bg", kind: ValueKind::Color, required: false, doc: "Background color" },
        ],
        doc: "Sheet row-level properties",
    },
    validate: None,
};
