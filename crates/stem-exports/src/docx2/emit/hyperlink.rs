//! Hyperlink + bookmark emission.
//!
//! Stem source:
//!
//!   `@link[to:"https://example.org"](click me)` — external
//!   `@link[to:"ref://_Toc_intro"](Introduction)` — anchor
//!   `@link[to:"#some-id"](See section)` — anchor (alt syntax)
//!
//! External targets become `<w:hyperlink r:id="rIdN">…</w:hyperlink>`
//! with a matching `Relationship` (TargetMode="External") in
//! `document.xml.rels`. Anchor targets become
//! `<w:hyperlink w:anchor="bookmark"/>` with no rel needed.
//!
//! Bookmarks are emitted with `bookmarkStart`/`bookmarkEnd` pairs.
//! Heading paragraphs receive a `_Toc_N` bookmark automatically so
//! the TOC field (task 12) can PAGEREF them.

use stem_core::ast::{Block, Body, TextPiece};

use super::super::xml::XmlBuf;
use super::ctx::EmitCtx;
use super::run::{self, RPr};

/// Resolved link target — either an in-package anchor or an
/// external URI.
pub enum LinkTarget {
    Anchor(String),
    External(String),
}

pub fn parse_target(s: &str) -> LinkTarget {
    if let Some(anchor) = s.strip_prefix("ref://") {
        return LinkTarget::Anchor(anchor.to_string());
    }
    if let Some(anchor) = s.strip_prefix('#') {
        return LinkTarget::Anchor(anchor.to_string());
    }
    LinkTarget::External(s.to_string())
}

/// Emit an `@link` inline. The visible runs come from the link's
/// body, walked recursively through `run::render_body_with` so
/// nested inlines (e.g. `@link(visit @text[weight:bold](X))`)
/// keep their formatting. The Hyperlink character style is always
/// applied on top.
pub fn render_link(b: &Block, ctx: &mut EmitCtx, parent: &RPr, x: &mut XmlBuf) {
    let Some(target_raw) = b.prop_str("to") else {
        // Malformed — fall back to plain runs so text doesn't drop.
        emit_link_body(b, ctx, parent, x);
        return;
    };
    let target = parse_target(target_raw);
    let link_rpr = parent.merged(&RPr {
        r_style: Some("Hyperlink".into()),
        ..Default::default()
    });
    match target {
        LinkTarget::External(url) => {
            let rid = ctx.add_external_link(&url);
            x.elem(
                "w:hyperlink",
                &[("r:id", &rid), ("w:history", "1")],
                |x| emit_link_body(b, ctx, &link_rpr, x),
            );
        }
        LinkTarget::Anchor(name) => {
            x.elem(
                "w:hyperlink",
                &[("w:anchor", &name), ("w:history", "1")],
                |x| emit_link_body(b, ctx, &link_rpr, x),
            );
        }
    }
}

fn emit_link_body(b: &Block, ctx: &mut EmitCtx, rpr: &RPr, x: &mut XmlBuf) {
    // Iterate the link's body via the same run dispatcher used
    // everywhere else. If the body is empty, emit a single run
    // carrying the visible text (defaulting to the target URL)
    // so the user sees something.
    let has_text = matches!(&b.body, Body::Text(pieces) if !pieces.is_empty());
    if has_text {
        run::render_body_with(b, ctx, x, rpr);
    } else {
        let fallback = b.prop_str("to").unwrap_or("link");
        emit_plain_run(fallback, rpr, x);
    }
}

fn emit_plain_run(text: &str, rpr: &RPr, x: &mut XmlBuf) {
    x.elem("w:r", &[], |x| {
        rpr.render(x);
        x.elem_text("w:t", &[], text, true);
    });
}

/// Emit a `<w:bookmarkStart>` + `<w:bookmarkEnd>` pair around
/// content produced by the closure. Allocates a fresh id from the
/// emit context.
pub fn render_bookmark(
    name: &str,
    ctx: &mut EmitCtx,
    x: &mut XmlBuf,
    inner: impl FnOnce(&mut XmlBuf),
) {
    let id = ctx.alloc_bookmark_id();
    let id_s = id.to_string();
    x.empty(
        "w:bookmarkStart",
        &[("w:id", &id_s), ("w:name", name)],
    );
    inner(x);
    x.empty("w:bookmarkEnd", &[("w:id", &id_s)]);
}

/// Convenience: flatten the link body's text — used for fallback
/// or for TOC entry visible text.
pub fn flatten_link_text(b: &Block) -> String {
    let mut out = String::new();
    if let Body::Text(pieces) = &b.body {
        for p in pieces {
            match p {
                TextPiece::Literal { text, .. } => out.push_str(text),
                TextPiece::Inline(inner) => out.push_str(&flatten_link_text(inner)),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use stem_parser::parse;

    use super::*;

    fn first_block(src: &str) -> Block {
        parse(src).document.blocks.first().unwrap().clone()
    }

    #[test]
    fn parse_target_recognizes_ref_and_hash() {
        match parse_target("ref://_Toc_1") {
            LinkTarget::Anchor(s) => assert_eq!(s, "_Toc_1"),
            _ => panic!("expected anchor"),
        }
        match parse_target("#bm") {
            LinkTarget::Anchor(s) => assert_eq!(s, "bm"),
            _ => panic!("expected anchor"),
        }
        match parse_target("https://example.org") {
            LinkTarget::External(s) => assert_eq!(s, "https://example.org"),
            _ => panic!("expected external"),
        }
    }

    #[test]
    fn external_link_registers_rid_and_emits_w_hyperlink() {
        let b = first_block(r#"p(visit @link[to:"https://example.org"](click) please)"#);
        let inline = match &b.body {
            Body::Text(pieces) => pieces
                .iter()
                .find_map(|p| match p {
                    TextPiece::Inline(blk) if blk.name == "link" => Some(blk.clone()),
                    _ => None,
                })
                .unwrap(),
            _ => panic!(),
        };
        let mut ctx = EmitCtx::new(None, 7);
        let mut x = XmlBuf::new();
        render_link(&inline, &mut ctx, &RPr::default(), &mut x);
        let s = x.finish();
        assert_eq!(ctx.hyperlinks.len(), 1);
        assert_eq!(ctx.hyperlinks[0].rid, "rId7");
        assert_eq!(ctx.hyperlinks[0].url, "https://example.org");
        assert!(s.contains(r#"<w:hyperlink r:id="rId7""#));
        assert!(s.contains(r#"<w:rStyle w:val="Hyperlink"/>"#));
        assert!(s.contains("click"));
    }

    #[test]
    fn anchor_link_emits_w_anchor_without_rel() {
        let b = first_block(r#"p(see @link[to:"ref://_Toc_1"](Section 1))"#);
        let inline = match &b.body {
            Body::Text(pieces) => pieces
                .iter()
                .find_map(|p| match p {
                    TextPiece::Inline(blk) if blk.name == "link" => Some(blk.clone()),
                    _ => None,
                })
                .unwrap(),
            _ => panic!(),
        };
        let mut ctx = EmitCtx::new(None, 7);
        let mut x = XmlBuf::new();
        render_link(&inline, &mut ctx, &RPr::default(), &mut x);
        let s = x.finish();
        assert!(ctx.hyperlinks.is_empty());
        assert!(s.contains(r#"<w:hyperlink w:anchor="_Toc_1""#));
    }

    #[test]
    fn external_links_dedupe_by_url() {
        let mut ctx = EmitCtx::new(None, 1);
        let r1 = ctx.add_external_link("https://example.org");
        let r2 = ctx.add_external_link("https://example.org");
        let r3 = ctx.add_external_link("https://other.org");
        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
        assert_eq!(ctx.hyperlinks.len(), 2);
    }

    #[test]
    fn bookmark_emits_start_and_end_with_matching_ids() {
        let mut ctx = EmitCtx::new(None, 1);
        let mut x = XmlBuf::new();
        render_bookmark("_Toc_intro", &mut ctx, &mut x, |x| {
            x.elem("w:r", &[], |x| {
                x.elem_text("w:t", &[], "Introduction", false);
            });
        });
        let s = x.finish();
        assert!(s.contains(r#"<w:bookmarkStart w:id="1" w:name="_Toc_intro"/>"#));
        assert!(s.contains(r#"<w:bookmarkEnd w:id="1"/>"#));
        // Start before content before end.
        let start = s.find("bookmarkStart").unwrap();
        let intro = s.find("Introduction").unwrap();
        let end = s.find("bookmarkEnd").unwrap();
        assert!(start < intro && intro < end);
    }

    #[test]
    fn empty_external_link_falls_back_to_url_text() {
        let b = first_block(r#"p(@link[to:"https://example.org"])"#);
        let inline = match &b.body {
            Body::Text(pieces) => pieces
                .iter()
                .find_map(|p| match p {
                    TextPiece::Inline(blk) if blk.name == "link" => Some(blk.clone()),
                    _ => None,
                })
                .unwrap(),
            _ => panic!(),
        };
        let mut ctx = EmitCtx::new(None, 7);
        let mut x = XmlBuf::new();
        render_link(&inline, &mut ctx, &RPr::default(), &mut x);
        let s = x.finish();
        assert!(s.contains("https://example.org"));
    }
}
