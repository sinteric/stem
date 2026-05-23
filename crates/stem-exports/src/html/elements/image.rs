//! `image` — figure with optional caption.
//!
//! Honors the docx property surface: `src`, `w`, `h`, `align`,
//! `alt`, `caption`, `before`, `after`, `line`, `float`. The
//! `float:anchor|behind` modes have no HTML equivalent (no page-
//! anchored frames in CSS) so they degrade to inline + centered,
//! matching the architecture decision the docx migration codified:
//! every property is a default the next layer can override, and
//! when a renderer can't honor an override it falls back to the
//! visible-closest default rather than erroring.
//!
//! Caption numbering is consumed from the per-document
//! [`HtmlCtx`] caption sequence so figure numbers stay aligned
//! across renderers given identical source.

use stem_core::ast::Block;

use super::super::ctx::HtmlCtx;
use super::super::{html_attr, html_text};
use super::block_props::paragraph_attrs;
use crate::style_props::parse_length_to_points;
use std::fmt::Write;

pub fn render_with_ctx(
    out: &mut String,
    b: &Block,
    ctx: &HtmlCtx,
) -> Result<(), std::fmt::Error> {
    let src = b.prop_str("src").unwrap_or("");
    let alt = b.prop_str("alt").unwrap_or("");

    // Figure-level CSS: spacing + alignment come from the same
    // shared property block paragraphs use. `align` defaults to
    // `center` so inline images sit visually under their centered
    // caption (the docx default).
    let attrs = paragraph_attrs(b);
    let mut fig_style = attrs.style;
    if !attrs.align_set {
        fig_style.insert_str(0, "text-align:center;");
    }

    write!(out, "<figure")?;
    if !fig_style.is_empty() {
        write!(out, " style=\"{}\"", fig_style)?;
    }
    writeln!(out, ">")?;

    write!(
        out,
        "<img src=\"{}\" alt=\"{}\"",
        html_attr(src),
        html_attr(alt)
    )?;
    // Width / height get inline styles so the values stay in their
    // authored units (`pt`, `in`, etc.). HTML attribute width/height
    // only accept px / %, so the style attribute is the safer path.
    let mut img_style = String::new();
    if let Some(w) = b.prop_str("w").and_then(parse_length_to_points) {
        let _ = write!(img_style, "width:{}pt;", fmt_pt(w));
    }
    if let Some(h) = b.prop_str("h").and_then(parse_length_to_points) {
        let _ = write!(img_style, "height:{}pt;", fmt_pt(h));
    }
    if !img_style.is_empty() {
        write!(out, " style=\"{}\"", img_style)?;
    }
    writeln!(out, ">")?;

    if let Some(text) = b.prop_str("caption") {
        let n = ctx.next_figure_caption();
        let bookmark = format!("_Toc_figure_{n}");
        writeln!(
            out,
            "<figcaption id=\"{}\">Figure {}. {}</figcaption>",
            html_attr(&bookmark),
            n,
            html_text(text)
        )?;
    }
    writeln!(out, "</figure>")?;
    Ok(())
}

fn fmt_pt(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.3}");
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        trimmed.to_string()
    }
}
