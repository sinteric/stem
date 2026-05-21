//! `named` — named range usable from formulas.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const NAMED: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "named",
        categories: &[Category::BlockMarker],
        doc_types: &[DocumentType::Sheet],
        bodies: &[BodyKind::None],
        parents: &["sheet"],
        children: &[],
        properties: &[
            PropertyDef { name: "name", kind: ValueKind::String, required: true, doc: "Name for formulas" },
            PropertyDef { name: "at", kind: ValueKind::Address, required: true, doc: "Range" },
        ],
        doc: "Named range",
    },
    validate: None,
};
