//! `cell` — two distinct schemas:
//!
//! * Document table cell (child of `row`).
//! * Sheet cell — value or formatting override (child of `sheet`, address-bearing).

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const CELL_DOC: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "cell",
        // BlockLeaf for the common single-line `cell(text)` form and
        // BlockContainer for `cell{ p(...) p(...) }` multi-paragraph
        // bodies (used by the docx exporter to mirror Word's
        // multi-paragraph table cells).
        categories: &[Category::BlockLeaf, Category::BlockContainer],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Text, BodyKind::Children],
        parents: &["row"],
        children: &["p"],
        properties: &[
            PropertyDef { name: "colspan", kind: ValueKind::Integer, required: false, doc: "Column span" },
            PropertyDef { name: "rowspan", kind: ValueKind::Integer, required: false, doc: "Row span" },
            PropertyDef { name: "bg", kind: ValueKind::Color, required: false, doc: "Background color" },
            PropertyDef {
                name: "align",
                kind: ValueKind::Enum(&["left", "center", "right", "justify"]),
                required: false,
                doc: "Horizontal alignment",
            },
            PropertyDef {
                name: "valign",
                kind: ValueKind::Enum(&["top", "middle", "bottom"]),
                required: false,
                doc: "Vertical alignment",
            },
        ],
        doc: "Table cell",
    },
    validate: None,
};

pub const CELL_SHEET: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "cell",
        categories: &[Category::BlockLeaf, Category::BlockMarker],
        doc_types: &[DocumentType::Sheet],
        bodies: &[BodyKind::Text, BodyKind::None],
        parents: &["sheet"],
        children: &[],
        properties: &[
            PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Cell address (A1, B5)" },
            PropertyDef {
                name: "fmt",
                kind: ValueKind::Enum(&["number", "currency", "percent", "date", "datetime", "text"]),
                required: false,
                doc: "Number format",
            },
            PropertyDef { name: "bg", kind: ValueKind::Color, required: false, doc: "Background color" },
            PropertyDef {
                name: "align",
                kind: ValueKind::Enum(&["left", "center", "right", "justify"]),
                required: false,
                doc: "Horizontal alignment",
            },
            PropertyDef {
                name: "weight",
                kind: ValueKind::Enum(&["light", "regular", "bold"]),
                required: false,
                doc: "Font weight",
            },
        ],
        doc: "Sheet cell — value or formatting override",
    },
    validate: None,
};
