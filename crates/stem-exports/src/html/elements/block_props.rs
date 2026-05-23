//! Shared paragraph-level property → CSS translation.
//!
//! Multiple block elements (`p`, `title`, `h1..h6`, `blockquote`,
//! `image` wrapping `<figure>`) accept the same per-block property
//! surface that the docx exporter does: `align`, `before`, `after`,
//! `line`, `size`, `border-top`. This module turns those into a
//! CSS declaration string each renderer concatenates into the
//! element's `style="…"` attribute.
//!
//! The `tabs:` property has no useful HTML representation (no flow
//! tab stops in CSS) and is silently dropped — matches the
//! ignore-unknown baseline.

use std::fmt::Write;

use stem_core::ast::Block;

use crate::style_props::{
    map_align_css, parse_length_to_points, parse_line, LineHeight,
};

#[derive(Default)]
pub struct ParagraphAttrs {
    /// CSS declarations for the wrapping element's `style` attribute.
    pub style: String,
    /// True when the block has `align:` set — useful for renderers
    /// like the image figure that need to know whether the source
    /// opted out of the centered default.
    pub align_set: bool,
}

pub fn paragraph_attrs(b: &Block) -> ParagraphAttrs {
    let mut s = String::new();
    let align_set = b.prop_str("align").is_some();
    if let Some(a) = b.prop_str("align").and_then(map_align_css) {
        let _ = write!(s, "text-align:{};", a);
    }
    if let Some(pt) = b.prop_str("before").and_then(parse_length_to_points) {
        let _ = write!(s, "margin-top:{};", fmt_pt(pt));
    }
    if let Some(pt) = b.prop_str("after").and_then(parse_length_to_points) {
        let _ = write!(s, "margin-bottom:{};", fmt_pt(pt));
    }
    if let Some(lh) = b.prop_str("line").and_then(parse_line) {
        match lh {
            LineHeight::Multiple(m) => {
                let _ = write!(s, "line-height:{};", fmt_num(m));
            }
            LineHeight::Points(p) => {
                let _ = write!(s, "line-height:{};", fmt_pt(p));
            }
        }
    }
    if let Some(pt) = b.prop_str("size").and_then(parse_length_to_points) {
        let _ = write!(s, "font-size:{};", fmt_pt(pt));
    }
    if matches!(b.prop_str("border-top"), Some("true" | "yes" | "on")) {
        s.push_str("border-top:1px solid currentColor;padding-top:4pt;");
    }
    ParagraphAttrs { style: s, align_set }
}

fn fmt_pt(v: f64) -> String {
    format!("{}pt", fmt_num(v))
}

fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use stem_parser::parse;

    use super::*;

    fn first_block(src: &str) -> Block {
        let r = parse(src);
        r.document.blocks.into_iter().next().expect("one block")
    }

    #[test]
    fn align_maps_to_text_align() {
        let s = paragraph_attrs(&first_block("p[align:center](x)")).style;
        assert!(s.contains("text-align:center;"));
    }

    #[test]
    fn before_after_emit_pt_margins() {
        let s = paragraph_attrs(&first_block("p[before:6pt, after:12pt](x)")).style;
        assert!(s.contains("margin-top:6pt;"));
        assert!(s.contains("margin-bottom:12pt;"));
    }

    #[test]
    fn line_multiplier_is_unitless() {
        let s = paragraph_attrs(&first_block("p[line:1.5x](x)")).style;
        assert!(s.contains("line-height:1.5;"));
    }

    #[test]
    fn line_points_carries_unit() {
        let s = paragraph_attrs(&first_block("p[line:18pt](x)")).style;
        assert!(s.contains("line-height:18pt;"));
    }

    #[test]
    fn size_maps_to_font_size() {
        let s = paragraph_attrs(&first_block("p[size:14pt](x)")).style;
        assert!(s.contains("font-size:14pt;"));
    }

    #[test]
    fn border_top_emits_rule_and_padding() {
        let s = paragraph_attrs(&first_block("p[border-top:true](x)")).style;
        assert!(s.contains("border-top:1px solid currentColor;"));
        assert!(s.contains("padding-top:4pt;"));
    }
}
