//! `caption` — standalone block caption.
//!
//! Most captions are properties of `table` or `image`. This element
//! is for the unusual case of a Caption-styled paragraph that doesn't
//! belong to either — placeholder positions, inline figure markers,
//! etc. It renders with the same Caption style as table/image
//! captions but doesn't carry a SEQ field by default.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const CAPTION: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "caption",
        categories: &[Category::BlockLeaf],
        doc_types: &[DocumentType::Document],
        bodies: &[BodyKind::Text, BodyKind::None],
        parents: &["any-block-container"],
        children: &[],
        properties: &[PropertyDef {
            name: "kind",
            kind: ValueKind::Enum(&["table", "figure", "none"]),
            required: false,
            doc: "Caption category — controls whether a SEQ field is emitted",
        }],
        doc: "Standalone caption paragraph",
    },
    validate: None,
};
