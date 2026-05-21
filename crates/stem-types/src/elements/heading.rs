//! `h1`..`h6` — document headings. All six share the same property set
//! and differ only in `name`.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

const HEADING_PROPS: &[PropertyDef] = &[
    PropertyDef {
        name: "id",
        kind: ValueKind::String,
        required: false,
        doc: "Heading id for cross-refs; auto-derived from text if absent",
    },
    PropertyDef {
        name: "numbered",
        kind: ValueKind::Bool,
        required: false,
        doc: "Include in the auto-numbering scheme",
    },
];

macro_rules! heading {
    ($const_name:ident, $name:literal) => {
        pub const $const_name: ElementDef = ElementDef {
            schema: ElementSchema {
                name: $name,
                categories: &[Category::BlockLeaf],
                doc_types: &[DocumentType::Document],
                bodies: &[BodyKind::Text],
                parents: &["root", "any-block-container"],
                children: &[],
                properties: HEADING_PROPS,
                doc: "Heading",
            },
            validate: None,
        };
    };
}

heading!(H1, "h1");
heading!(H2, "h2");
heading!(H3, "h3");
heading!(H4, "h4");
heading!(H5, "h5");
heading!(H6, "h6");
