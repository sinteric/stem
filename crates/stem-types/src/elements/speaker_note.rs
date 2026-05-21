//! `speaker-note` — hidden presenter notes for a slide.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema};

pub const SPEAKER_NOTE: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "speaker-note",
        categories: &[Category::BlockLeaf],
        doc_types: &[DocumentType::Presentation],
        bodies: &[BodyKind::Text],
        parents: &["slide"],
        children: &[],
        properties: &[],
        doc: "Speaker notes",
    },
    validate: None,
};
