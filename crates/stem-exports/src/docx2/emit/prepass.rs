//! Pre-walk the cooked AST and populate the heading + caption
//! registries on [`EmitCtx`].
//!
//! The TOC field (task 12) sits at the *start* of the document
//! but renders entries for every heading downstream. Walking the
//! AST once up front gives the TOC builder the full anchor list
//! before any paragraph emits.

use stem_core::ast::{Block, Body, Document, TextPiece};

use super::ctx::{CaptionAnchor, CaptionKind, EmitCtx, HeadingAnchor};

/// Populate `ctx.heading_anchors` and `ctx.captions` from `doc`.
pub fn collect(doc: &Document, ctx: &mut EmitCtx) {
    let mut table_seq = 0u32;
    let mut figure_seq = 0u32;
    walk(&doc.blocks, ctx, &mut table_seq, &mut figure_seq);
}

fn walk(blocks: &[Block], ctx: &mut EmitCtx, table_seq: &mut u32, figure_seq: &mut u32) {
    for b in blocks {
        match b.name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level: u32 = b.name[1..].parse().unwrap_or(1);
                let idx = ctx.heading_anchors.len() + 1;
                ctx.heading_anchors.push(HeadingAnchor {
                    bookmark: format!("_Toc{idx}"),
                    level,
                    text: flatten_text(b),
                });
            }
            "table" => {
                if let Some(text) = b.prop_str("caption") {
                    *table_seq += 1;
                    ctx.captions.push(CaptionAnchor {
                        kind: CaptionKind::Table,
                        bookmark: format!("_Toc_table_{}", *table_seq),
                        text: text.to_string(),
                        seq: *table_seq,
                    });
                }
                if let Body::Children(children) = &b.body {
                    walk(children, ctx, table_seq, figure_seq);
                }
            }
            "image" => {
                if let Some(text) = b.prop_str("caption") {
                    *figure_seq += 1;
                    ctx.captions.push(CaptionAnchor {
                        kind: CaptionKind::Figure,
                        bookmark: format!("_Toc_figure_{}", *figure_seq),
                        text: text.to_string(),
                        seq: *figure_seq,
                    });
                }
            }
            // Container-shaped blocks — recurse to find headings
            // and captions nested inside them.
            "section" => {
                if let Body::Children(children) = &b.body {
                    walk(children, ctx, table_seq, figure_seq);
                }
            }
            // Header and footer blocks become separate parts —
            // capture their children (the contents that will go
            // into headerN.xml / footerN.xml) and do NOT recurse
            // into them for heading/caption collection (page
            // chrome shouldn't contribute to the TOC).
            "header" => {
                if let Body::Children(children) = &b.body {
                    ctx.headers.push(children.clone());
                }
            }
            "footer" => {
                if let Body::Children(children) = &b.body {
                    ctx.footers.push(children.clone());
                }
            }
            _ => {
                if let Body::Children(children) = &b.body {
                    walk(children, ctx, table_seq, figure_seq);
                }
            }
        }
    }
}

fn flatten_text(b: &Block) -> String {
    let mut out = String::new();
    if let Body::Text(pieces) = &b.body {
        for p in pieces {
            match p {
                TextPiece::Literal { text, .. } => out.push_str(text),
                TextPiece::Inline(inner) => out.push_str(&flatten_text(inner)),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use stem_parser::parse;

    use super::*;

    fn collect_from(src: &str) -> EmitCtx {
        let r = parse(src);
        let mut ctx = EmitCtx::new(None, 1);
        collect(&r.document, &mut ctx);
        ctx
    }

    #[test]
    fn headings_collected_in_order_with_toc_bookmarks() {
        let ctx = collect_from(
            r#"h1(Alpha)
h2(Beta)
h3(Gamma)
h1(Delta)"#,
        );
        assert_eq!(ctx.heading_anchors.len(), 4);
        assert_eq!(ctx.heading_anchors[0].bookmark, "_Toc1");
        assert_eq!(ctx.heading_anchors[0].text, "Alpha");
        assert_eq!(ctx.heading_anchors[0].level, 1);
        assert_eq!(ctx.heading_anchors[1].bookmark, "_Toc2");
        assert_eq!(ctx.heading_anchors[1].level, 2);
        assert_eq!(ctx.heading_anchors[3].bookmark, "_Toc4");
        assert_eq!(ctx.heading_anchors[3].level, 1);
    }

    #[test]
    fn captions_collect_separately_by_kind() {
        let ctx = collect_from(
            r#"table[caption:"First table"]{ row{ cell(x) } }
image[src:"a.png", caption:"First fig"]
table[caption:"Second table"]{ row{ cell(y) } }
image[src:"b.png", caption:"Second fig"]"#,
        );
        let tables: Vec<&CaptionAnchor> = ctx
            .captions
            .iter()
            .filter(|c| matches!(c.kind, CaptionKind::Table))
            .collect();
        let figures: Vec<&CaptionAnchor> = ctx
            .captions
            .iter()
            .filter(|c| matches!(c.kind, CaptionKind::Figure))
            .collect();
        assert_eq!(tables.len(), 2);
        assert_eq!(figures.len(), 2);
        assert_eq!(tables[0].text, "First table");
        assert_eq!(tables[0].bookmark, "_Toc_table_1");
        assert_eq!(figures[1].bookmark, "_Toc_figure_2");
    }

    #[test]
    fn headings_inside_section_are_found_recursively() {
        let ctx = collect_from(
            r#"section{
  h1(Outer)
  section{
    h2(Inner)
  }
}"#,
        );
        assert_eq!(ctx.heading_anchors.len(), 2);
        assert_eq!(ctx.heading_anchors[0].text, "Outer");
        assert_eq!(ctx.heading_anchors[1].text, "Inner");
    }
}
