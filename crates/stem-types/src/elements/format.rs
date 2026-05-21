//! `format` — range formatting without values.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const FORMAT: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "format",
        categories: &[Category::BlockMarker],
        doc_types: &[DocumentType::Sheet],
        bodies: &[BodyKind::None],
        parents: &["sheet"],
        children: &[],
        properties: &[
            PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Range" },
            PropertyDef { name: "bg", kind: ValueKind::Color, required: false, doc: "Background color" },
            PropertyDef {
                name: "weight",
                kind: ValueKind::Enum(&["light", "regular", "bold"]),
                required: false,
                doc: "Font weight",
            },
            PropertyDef {
                name: "align",
                kind: ValueKind::Enum(&["left", "center", "right", "justify"]),
                required: false,
                doc: "Horizontal alignment",
            },
            PropertyDef {
                name: "fmt",
                kind: ValueKind::Enum(&["number", "currency", "percent", "date", "datetime", "text"]),
                required: false,
                doc: "Number format",
            },
        ],
        doc: "Range formatting without values",
    },
    validate: None,
};
