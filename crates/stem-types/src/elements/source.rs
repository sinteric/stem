//! `source` — external CSV reference for a sheet.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const SOURCE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "source",
        categories: &[Category::BlockMarker],
        doc_types: &[DocumentType::Sheet],
        bodies: &[BodyKind::None],
        parents: &["sheet"],
        children: &[],
        properties: &[
            PropertyDef { name: "file", kind: ValueKind::String, required: true, doc: "Path to CSV file" },
            PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Top-left anchor" },
            PropertyDef { name: "sep", kind: ValueKind::String, required: false, doc: "Cell separator" },
            PropertyDef { name: "has-header", kind: ValueKind::Bool, required: false, doc: "Treat first row as header" },
            PropertyDef { name: "encoding", kind: ValueKind::String, required: false, doc: "Source file encoding" },
        ],
        doc: "External CSV reference",
    },
    validate: None,
};
