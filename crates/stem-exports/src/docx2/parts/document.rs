//! `word/document.xml` — the body of the docx.
//!
//! Task 6 walks the cooked AST and emits one paragraph per
//! top-level block, dispatching to the per-shape emitters in
//! [`super::super::emit`].

use stem_core::ast::Document;

use super::super::emit::paragraph;
use super::super::xml::{Ns, XmlBuf};

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// Document body for the cooked AST. Standard letter-size section
/// properties land at the end; subsequent tasks may add header/
/// footer references and per-section overrides via the `sectPr`
/// builder.
pub fn body(doc: &Document) -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem_with_ns(
        "w:document",
        &[Ns { prefix: "w", uri: NS_W }],
        &[],
        |x| {
            x.elem("w:body", &[], |x| {
                for block in &doc.blocks {
                    paragraph::render_block(block, x);
                }
                render_sect_pr(x);
            });
        },
    );
    x.finish()
}

/// Original minimal body — a single empty paragraph + section
/// properties. Kept for the docx2 test that needs a known-fixed
/// reference (and for the dev-only `STEM_DOCX2_DUMP` smoke check
/// when the caller passes no source).
pub fn minimal() -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem_with_ns(
        "w:document",
        &[Ns { prefix: "w", uri: NS_W }],
        &[],
        |x| {
            x.elem("w:body", &[], |x| {
                x.empty("w:p", &[]);
                render_sect_pr(x);
            });
        },
    );
    x.finish()
}

fn render_sect_pr(x: &mut XmlBuf) {
    x.elem("w:sectPr", &[], |x| {
        x.empty("w:pgSz", &[("w:w", "12240"), ("w:h", "15840")]);
        x.empty(
            "w:pgMar",
            &[
                ("w:top", "1440"),
                ("w:right", "1440"),
                ("w:bottom", "1440"),
                ("w:left", "1440"),
                ("w:header", "720"),
                ("w:footer", "720"),
                ("w:gutter", "0"),
            ],
        );
    });
}
