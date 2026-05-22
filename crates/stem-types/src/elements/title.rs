//! `title` — slide title (presentation) or cover-page title line
//! (document). Document titles render with the built-in Word `Title`
//! paragraph style: larger and centered, used for the front-page
//! authorship block.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema, PropertyDef, ValueKind};

pub const TITLE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "title",
        categories: &[Category::BlockLeaf],
        doc_types: &[DocumentType::Document, DocumentType::Presentation],
        bodies: &[BodyKind::Text],
        parents: &["root", "slide"],
        children: &[],
        properties: &[
            PropertyDef {
                name: "size",
                kind: ValueKind::Length,
                required: false,
                doc: "Override default title size",
            },
            PropertyDef {
                name: "align",
                kind: ValueKind::Enum(&["left", "center", "right", "justify"]),
                required: false,
                doc: "Horizontal alignment",
            },
        ],
        doc: "Title — slide title (presentation) or cover-page line (document)",
    },
    validate: None,
};
