//! `word/document.xml` — the body of the docx.
//!
//! Task 6 walks the cooked AST and emits one paragraph per
//! top-level block, dispatching to the per-shape emitters in
//! [`super::super::emit`].

use stem_core::ast::Document;

use super::super::emit::ctx::{EmitCtx, HeaderFooterScope};
use super::super::emit::{paragraph, prepass};
use super::super::xml::{Ns, XmlBuf};

/// Page geometry resolved from document metadata. All values
/// are in dxa (twentieths of a point). Defaults match Word's
/// "Letter, 1in margins" preset.
#[derive(Clone, Copy)]
pub struct PageGeometry {
    pub width: u32,
    pub height: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
    pub header_offset: u32,
    pub footer_offset: u32,
}

impl Default for PageGeometry {
    fn default() -> Self {
        Self {
            width: 12240,  // letter
            height: 15840,
            top: 1440,
            right: 1440,
            bottom: 1440,
            left: 1440,
            header_offset: 720,
            footer_offset: 720,
        }
    }
}

impl PageGeometry {
    /// Read page geometry from the metadata header.
    ///
    /// Recognized keys (units accepted everywhere: pt / in / cm /
    /// mm / px / bare-number = pt):
    /// - `page-size`: `letter` / `a4` / `legal`, or `WIDTHxHEIGHT`
    /// - `margin`: shorthand — `1in`, or four-value
    ///   `"top right bottom left"` (with explicit units)
    /// - `margin-top`, `margin-right`, `margin-bottom`, `margin-left`
    /// - `header`, `footer`: header/footer offset from the page
    ///   edge (matches OOXML's `w:header` / `w:footer`)
    pub fn from_metadata(doc: &Document) -> Self {
        let mut g = Self::default();
        let m = &doc.metadata;
        if let Some(v) = m.get_str("page-size") {
            if let Some((w, h)) = parse_page_size(v) {
                g.width = w;
                g.height = h;
            }
        }
        // Shorthand `margin: ...` — single value applies to all
        // four sides; four space-separated values are
        // top/right/bottom/left (CSS order).
        if let Some(v) = m.get_str("margin") {
            let parts: Vec<u32> = v
                .split_whitespace()
                .filter_map(super::super::emit::paragraph::parse_dxa)
                .collect();
            match parts.len() {
                1 => {
                    g.top = parts[0];
                    g.right = parts[0];
                    g.bottom = parts[0];
                    g.left = parts[0];
                }
                4 => {
                    g.top = parts[0];
                    g.right = parts[1];
                    g.bottom = parts[2];
                    g.left = parts[3];
                }
                _ => {}
            }
        }
        for (key, slot) in [
            ("margin-top", &mut g.top),
            ("margin-right", &mut g.right),
            ("margin-bottom", &mut g.bottom),
            ("margin-left", &mut g.left),
            ("header", &mut g.header_offset),
            ("footer", &mut g.footer_offset),
        ] {
            if let Some(v) = m.get_str(key) {
                if let Some(dxa) = super::super::emit::paragraph::parse_dxa(v) {
                    *slot = dxa;
                }
            }
        }
        g
    }
}

fn parse_page_size(s: &str) -> Option<(u32, u32)> {
    let s = s.trim();
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "letter" => return Some((12240, 15840)),
        "legal" => return Some((12240, 20160)),
        "a4" => return Some((11906, 16838)),
        "a5" => return Some((8392, 11906)),
        _ => {}
    }
    let (w, h) = s.split_once('x').or_else(|| s.split_once('X'))?;
    let parse = super::super::emit::paragraph::parse_dxa;
    Some((parse(w.trim())?, parse(h.trim())?))
}

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
    let geo = PageGeometry::from_metadata(doc);
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
                    &geo,
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
/// properties. Kept for the docx test that needs a known-fixed
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
                render_sect_pr(x, &PageGeometry::default());
            });
        },
    );
    x.finish()
}

fn render_sect_pr(x: &mut XmlBuf, geo: &PageGeometry) {
    render_sect_pr_with_refs(x, geo, &[], &[], &[], &[]);
}

fn render_sect_pr_with_refs(
    x: &mut XmlBuf,
    geo: &PageGeometry,
    header_rids: &[String],
    header_scopes: &[HeaderFooterScope],
    footer_rids: &[String],
    footer_scopes: &[HeaderFooterScope],
) {
    let has_first = header_scopes
        .iter()
        .chain(footer_scopes.iter())
        .any(|s| matches!(s, HeaderFooterScope::First));
    let w_s = geo.width.to_string();
    let h_s = geo.height.to_string();
    let top_s = geo.top.to_string();
    let right_s = geo.right.to_string();
    let bottom_s = geo.bottom.to_string();
    let left_s = geo.left.to_string();
    let header_s = geo.header_offset.to_string();
    let footer_s = geo.footer_offset.to_string();
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
        x.empty("w:pgSz", &[("w:w", w_s.as_str()), ("w:h", h_s.as_str())]);
        x.empty(
            "w:pgMar",
            &[
                ("w:top", top_s.as_str()),
                ("w:right", right_s.as_str()),
                ("w:bottom", bottom_s.as_str()),
                ("w:left", left_s.as_str()),
                ("w:header", header_s.as_str()),
                ("w:footer", footer_s.as_str()),
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
