//! `col` — two distinct schemas:
//!
//! * Document/presentation layout column (child of `layout`).
//! * Sheet column-level properties (child of `sheet`, address-bearing).
//!
//! The validator's `(name, doc_type)` lookup picks the right one.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const COL_LAYOUT: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "col",
        categories: &[Category::BlockContainer],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Children],
        parents: &["layout"],
        children: &["any-block"],
        properties: &[PropertyDef {
            name: "width",
            kind: ValueKind::String,
            required: false,
            doc: "Width hint",
        }],
        doc: "One column inside a layout",
    },
    validate: None,
};

pub const COL_SHEET: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "col",
        categories: &[Category::BlockMarker],
        doc_types: &[DocumentType::Sheet],
        bodies: &[BodyKind::None],
        parents: &["sheet"],
        children: &[],
        properties: &[
            PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Column letter or range" },
            PropertyDef { name: "width", kind: ValueKind::Length, required: false, doc: "Column width" },
            PropertyDef {
                name: "fmt",
                kind: ValueKind::Enum(&["number", "currency", "percent", "date", "datetime", "text"]),
                required: false,
                doc: "Number format",
            },
        ],
        doc: "Sheet column-level properties",
    },
    validate: None,
};
