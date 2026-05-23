//! Per-document context threaded through the HTML render.
//!
//! Mirrors the docx prepass: a single walk of the cooked document
//! collects source-supplied `style[id:..., ...]` overrides, heading
//! bookmarks, and caption (Table/Figure) bookmarks before body
//! emission. The renderer then uses the context to:
//!
//! - Emit per-style CSS rules into the `<style>` block in `<head>`
//!   so authored overrides land alongside the format defaults.
//! - Stamp stable `id="_TocN"` bookmarks on headings so a TOC nav
//!   can link back to them.
//! - Prefix captions with auto-numbered `"Figure N. "` / `"Table N. "`
//!   and bookmark them so a list-of-figures / list-of-tables nav can
//!   resolve.
//!
//! The bookmark-name and sequence-counter semantics are deliberately
//! identical to the docx side ([[docx2-migration-plan]] tasks 11-12).
//! Both renderers should produce the same anchor names from the same
//! source so cross-format links remain stable.
//!
//! `HtmlCtx` uses `Cell` for the live cursors — the per-element
//! render functions only see `&HtmlCtx` because the dispatch table
//! holds `fn` pointers, not closures. Interior mutability keeps the
//! API surface flat while still letting headings/captions consume
//! their next sequence number in document order.

use std::cell::Cell;

use stem_core::ast::{Block, Body, Document, TextPiece};
use stem_core::theme::Theme;

use crate::style_props::{
    normalize_hex_color, parse_length_to_points, parse_line, LineHeight,
};

/// One source-supplied style override patch. Mirrors the docx
/// `StyleOverride` shape but keeps values in source-natural units
/// so the CSS emitter can format them directly.
#[derive(Clone, Debug, Default)]
pub struct StyleOverride {
    pub id: String,
    pub before_pt: Option<f64>,
    pub after_pt: Option<f64>,
    pub line: Option<LineHeight>,
    pub align: Option<String>,
    pub size_pt: Option<f64>,
    pub color: Option<String>,
    pub bg: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub strike: Option<bool>,
    pub underline: Option<String>,
    pub font: Option<String>,
    pub border_top: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct HeadingAnchor {
    pub bookmark: String,
    pub level: u32,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptionKind {
    Table,
    Figure,
}

#[derive(Clone, Debug)]
pub struct CaptionAnchor {
    pub kind: CaptionKind,
    pub bookmark: String,
    pub text: String,
    pub seq: u32,
}

pub struct HtmlCtx<'a> {
    pub theme: &'a Theme,
    pub style_overrides: Vec<StyleOverride>,
    pub heading_anchors: Vec<HeadingAnchor>,
    pub caption_anchors: Vec<CaptionAnchor>,
    heading_cursor: Cell<usize>,
    table_caption_seq: Cell<u32>,
    figure_caption_seq: Cell<u32>,
}

impl<'a> HtmlCtx<'a> {
    pub fn new(doc: &Document, theme: &'a Theme) -> Self {
        let mut style_overrides = Vec::new();
        let mut heading_anchors = Vec::new();
        let mut caption_anchors = Vec::new();
        let mut t = 0u32;
        let mut f = 0u32;
        walk(
            &doc.blocks,
            &mut style_overrides,
            &mut heading_anchors,
            &mut caption_anchors,
            &mut t,
            &mut f,
        );
        Self {
            theme,
            style_overrides,
            heading_anchors,
            caption_anchors,
            heading_cursor: Cell::new(0),
            table_caption_seq: Cell::new(0),
            figure_caption_seq: Cell::new(0),
        }
    }

    /// Consume the next heading bookmark in document order. Returns
    /// `None` once the registry is exhausted — callers fall back to
    /// a deterministic synthesized name.
    pub fn next_heading_anchor(&self) -> Option<&HeadingAnchor> {
        let i = self.heading_cursor.get();
        let a = self.heading_anchors.get(i)?;
        self.heading_cursor.set(i + 1);
        Some(a)
    }

    pub fn next_table_caption(&self) -> u32 {
        let n = self.table_caption_seq.get() + 1;
        self.table_caption_seq.set(n);
        n
    }

    pub fn next_figure_caption(&self) -> u32 {
        let n = self.figure_caption_seq.get() + 1;
        self.figure_caption_seq.set(n);
        n
    }

    pub fn find_style_override(&self, id: &str) -> Option<&StyleOverride> {
        self.style_overrides.iter().find(|o| o.id == id)
    }
}

fn walk(
    blocks: &[Block],
    style_overrides: &mut Vec<StyleOverride>,
    heading_anchors: &mut Vec<HeadingAnchor>,
    caption_anchors: &mut Vec<CaptionAnchor>,
    table_seq: &mut u32,
    figure_seq: &mut u32,
) {
    for b in blocks {
        match b.name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level: u32 = b.name[1..].parse().unwrap_or(1);
                let idx = heading_anchors.len() + 1;
                heading_anchors.push(HeadingAnchor {
                    bookmark: format!("_Toc{idx}"),
                    level,
                    text: flatten_text(b),
                });
            }
            "table" => {
                if let Some(text) = b.prop_str("caption") {
                    *table_seq += 1;
                    caption_anchors.push(CaptionAnchor {
                        kind: CaptionKind::Table,
                        bookmark: format!("_Toc_table_{}", *table_seq),
                        text: text.to_string(),
                        seq: *table_seq,
                    });
                }
                if let Body::Children(children) = &b.body {
                    walk(
                        children,
                        style_overrides,
                        heading_anchors,
                        caption_anchors,
                        table_seq,
                        figure_seq,
                    );
                }
            }
            "image" => {
                if let Some(text) = b.prop_str("caption") {
                    *figure_seq += 1;
                    caption_anchors.push(CaptionAnchor {
                        kind: CaptionKind::Figure,
                        bookmark: format!("_Toc_figure_{}", *figure_seq),
                        text: text.to_string(),
                        seq: *figure_seq,
                    });
                }
            }
            "style" => {
                if let Some(id) = b.prop_str("id") {
                    style_overrides.push(parse_style_override(id, b));
                }
            }
            // Page chrome — these are docx-only concepts; don't
            // contribute to TOC / LoT / LoF collection.
            "header" | "footer" => {}
            _ => {
                if let Body::Children(children) = &b.body {
                    walk(
                        children,
                        style_overrides,
                        heading_anchors,
                        caption_anchors,
                        table_seq,
                        figure_seq,
                    );
                }
            }
        }
    }
}

fn parse_style_override(id: &str, b: &Block) -> StyleOverride {
    let mut o = StyleOverride {
        id: id.to_string(),
        ..Default::default()
    };
    o.before_pt = b.prop_str("before").and_then(parse_length_to_points);
    o.after_pt = b.prop_str("after").and_then(parse_length_to_points);
    o.line = b.prop_str("line").and_then(parse_line);
    o.align = b.prop_str("align").map(str::to_string);
    o.size_pt = b.prop_str("size").and_then(parse_length_to_points);
    o.color = b.prop_str("color").and_then(normalize_hex_color);
    o.bg = b.prop_str("bg").and_then(normalize_hex_color);
    o.bold = bool_prop(b, "bold");
    o.italic = bool_prop(b, "italic");
    o.strike = bool_prop(b, "strike");
    o.underline = b.prop_str("underline").map(str::to_string);
    o.font = b.prop_str("font").map(str::to_string);
    o.border_top = bool_prop(b, "border-top");
    o
}

fn bool_prop(b: &Block, key: &str) -> Option<bool> {
    b.prop_str(key).map(|v| matches!(v, "true" | "yes" | "on"))
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

    fn collect_from(src: &str) -> HtmlCtx<'static> {
        // SAFETY: leak the Theme so the returned ctx has a 'static
        // borrow — fine for the tiny test suite, where the Theme
        // outlives the test process.
        let theme: &'static Theme = Box::leak(Box::new(Theme::default()));
        let r = parse(src);
        HtmlCtx::new(&r.document, theme)
    }

    #[test]
    fn headings_collected_in_doc_order_with_toc_bookmarks() {
        let ctx = collect_from("h1(Alpha)\nh2(Beta)\nh1(Gamma)");
        assert_eq!(ctx.heading_anchors.len(), 3);
        assert_eq!(ctx.heading_anchors[0].bookmark, "_Toc1");
        assert_eq!(ctx.heading_anchors[0].level, 1);
        assert_eq!(ctx.heading_anchors[1].text, "Beta");
        assert_eq!(ctx.heading_anchors[2].bookmark, "_Toc3");
    }

    #[test]
    fn style_block_collects_overrides() {
        let ctx = collect_from(
            r##"style[id:Heading1, color:"#C0392B", size:20pt, after:200pt, bold:true]"##,
        );
        let o = ctx.find_style_override("Heading1").expect("override present");
        assert_eq!(o.color.as_deref(), Some("C0392B"));
        assert_eq!(o.size_pt, Some(20.0));
        assert_eq!(o.after_pt, Some(200.0));
        assert_eq!(o.bold, Some(true));
    }

    #[test]
    fn captions_split_table_and_figure() {
        let ctx = collect_from(
            r#"table[caption:"Alpha"]{ row{ cell(x) } }
image[src:"a.png", caption:"Pic"]
table[caption:"Beta"]{ row{ cell(y) } }"#,
        );
        let table_caps: Vec<&CaptionAnchor> = ctx
            .caption_anchors
            .iter()
            .filter(|c| matches!(c.kind, CaptionKind::Table))
            .collect();
        let figure_caps: Vec<&CaptionAnchor> = ctx
            .caption_anchors
            .iter()
            .filter(|c| matches!(c.kind, CaptionKind::Figure))
            .collect();
        assert_eq!(table_caps.len(), 2);
        assert_eq!(figure_caps.len(), 1);
        assert_eq!(table_caps[0].bookmark, "_Toc_table_1");
        assert_eq!(figure_caps[0].text, "Pic");
    }
}
