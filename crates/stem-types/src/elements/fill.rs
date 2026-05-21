//! `fill` — bulk inline CSV data for a sheet.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const FILL: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "fill",
        categories: &[Category::BlockLeaf],
        doc_types: &[DocumentType::Sheet],
        bodies: &[BodyKind::Text],
        parents: &["sheet"],
        children: &[],
        properties: &[
            PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Top-left anchor" },
            PropertyDef { name: "sep", kind: ValueKind::String, required: false, doc: "Cell separator" },
            PropertyDef { name: "has-header", kind: ValueKind::Bool, required: false, doc: "Treat first row as header" },
        ],
        doc: "Bulk inline data (CSV)",
    },
    validate: None,
};
