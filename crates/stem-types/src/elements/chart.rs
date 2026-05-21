//! `chart` — chart from a data range.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const CHART: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "chart",
        categories: &[Category::BlockMarker],
        doc_types: &[DocumentType::Document, DocumentType::Presentation, DocumentType::Sheet],
        bodies: &[BodyKind::None],
        parents: &["any-block-container"],
        children: &[],
        properties: &[
            PropertyDef {
                name: "type",
                kind: ValueKind::Enum(&["bar", "line", "pie", "scatter", "area"]),
                required: true,
                doc: "Chart type",
            },
            PropertyDef { name: "data", kind: ValueKind::String, required: true, doc: "Range ref" },
            PropertyDef { name: "title", kind: ValueKind::String, required: false, doc: "Chart title" },
        ],
        doc: "Chart from a data range",
    },
    validate: None,
};
