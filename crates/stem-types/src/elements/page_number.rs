//! `page-number` and `total-pages` — inline page-numbering fields.
//!
//! These render as live Word fields (PAGE / NUMPAGES) in paged output.
//! In non-paginated output (HTML) they may render as placeholders or
//! as the current/total page count if the renderer knows it.

use crate::element::ElementDef;
use crate::schema::{BodyKind, Category, DocumentType, ElementSchema};

// NOTE on body shape: the Stem grammar (§G1) requires every `@`-prefixed
// inline to carry at least `[…]` or `(…)`. So in source these always
// appear as `@page-number()` even though semantically they have no body.
// We declare `Text` here (allowing the empty `()`) and `None` (for
// completeness if a future grammar relaxation lets bare `@page-number`
// stand alone). The exporter ignores any text content it finds.
pub const PAGE_NUMBER: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "page-number",
        categories: &[Category::Inline],
        doc_types: &[DocumentType::Document],
        bodies: &[BodyKind::None, BodyKind::Text],
        parents: &["any-text-body"],
        children: &[],
        properties: &[],
        doc: "Current page number in paged output (Word PAGE field)",
    },
    validate: None,
};

pub const TOTAL_PAGES: ElementDef = ElementDef {
    schema: ElementSchema {
        name: "total-pages",
        categories: &[Category::Inline],
        doc_types: &[DocumentType::Document],
        bodies: &[BodyKind::None, BodyKind::Text],
        parents: &["any-text-body"],
        children: &[],
        properties: &[],
        doc: "Total page count in paged output (Word NUMPAGES field)",
    },
    validate: None,
};
