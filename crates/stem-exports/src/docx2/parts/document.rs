//! `word/document.xml` — the body of the docx.
//!
//! Task 6 walks the cooked AST and emits one paragraph per
//! top-level block, dispatching to the per-shape emitters in
//! [`super::super::emit`].

use stem_core::ast::Document;

use super::super::emit::ctx::{EmitCtx, HeaderFooterScope};
use super::super::emit::{paragraph, prepass};
use super::super::xml::{Ns, XmlBuf};

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

// `w:document` needs more namespaces declared once embedded
// drawings (`<w:drawing>` etc.) are referenced. We declare them
// on the root element so individual elements don't need to
// re-declare. Word ignores unused namespaces.
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";

/// Document body for the cooked AST. Walks the AST with the
/// shared [`EmitCtx`] so embedded images, hyperlinks, and the
/// other side-state needed by `document.xml.rels` accumulate
/// during emission. A prepass populates heading + caption
/// anchors first so the TOC field can render with full entries
/// even when it sits at the start of the document.
pub fn body(doc: &Document, ctx: &mut EmitCtx) -> String {
    prepass::collect(doc, ctx);
    // Pre-allocate header/footer rIds so sectPr can reference them
    // and the packager can produce `<Relationship>` entries with
    // matching ids.
    for _ in 0..ctx.headers.len() {
        let rid = ctx.alloc_rid();
        ctx.header_rids.push(rid);
    }
    for _ in 0..ctx.footers.len() {
        let rid = ctx.alloc_rid();
        ctx.footer_rids.push(rid);
    }
    let header_rids = ctx.header_rids.clone();
    let footer_rids = ctx.footer_rids.clone();
    let header_scopes = ctx.header_scopes.clone();
    let footer_scopes = ctx.footer_scopes.clone();
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem_with_ns(
        "w:document",
        &[
            Ns { prefix: "w", uri: NS_W },
            Ns { prefix: "r", uri: NS_R },
            Ns { prefix: "wp", uri: NS_WP },
            Ns { prefix: "a", uri: NS_A },
            Ns { prefix: "pic", uri: NS_PIC },
        ],
        &[],
        |x| {
            x.elem("w:body", &[], |x| {
                for block in &doc.blocks {
                    paragraph::render_block(block, ctx, x);
                }
                render_sect_pr_with_refs(
                    x,
                    &header_rids,
                    &header_scopes,
                    &footer_rids,
                    &footer_scopes,
                );
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
    render_sect_pr_with_refs(x, &[], &[], &[], &[]);
}

fn render_sect_pr_with_refs(
    x: &mut XmlBuf,
    header_rids: &[String],
    header_scopes: &[HeaderFooterScope],
    footer_rids: &[String],
    footer_scopes: &[HeaderFooterScope],
) {
    let has_first = header_scopes
        .iter()
        .chain(footer_scopes.iter())
        .any(|s| matches!(s, HeaderFooterScope::First));
    x.elem("w:sectPr", &[], |x| {
        // Header/footer references come before pgSz per the schema.
        for (rid, scope) in header_rids.iter().zip(header_scopes.iter()) {
            x.empty(
                "w:headerReference",
                &[("w:type", scope.w_type()), ("r:id", rid.as_str())],
            );
        }
        for (rid, scope) in footer_rids.iter().zip(footer_scopes.iter()) {
            x.empty(
                "w:footerReference",
                &[("w:type", scope.w_type()), ("r:id", rid.as_str())],
            );
        }
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
        // When a first-page header or footer exists, `<w:titlePg/>`
        // must be set so Word uses the "first" variant on page 1.
        if has_first {
            x.empty("w:titlePg", &[]);
        }
    });
}
