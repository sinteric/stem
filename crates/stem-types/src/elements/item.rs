//! `item` — slide bullet item.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema};

pub const ITEM: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "item",
        categories: &[Category::BlockLeaf, Category::BlockContainer],
        doc_types: &[DocumentType::Presentation],
        bodies: &[BodyKind::Text, BodyKind::Children],
        parents: &["bullets"],
        children: &["bullets"],
        properties: &[],
        doc: "Bullet item",
    },
    validate: None,
};
