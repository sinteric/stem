//! Translate per-style defaults + source-supplied
//! `style[id:..., ...]` overrides into a CSS block that the
//! exporter inlines into `<head>`.
//!
//! Mirrors `crates/stem-exports/src/docx/parts/styles.rs` semantics:
//! every recognized style id (`Normal`, `Heading1..6`, `Title`,
//! `Caption`, `Hyperlink`, `FootnoteReference`, `TOC1..9`,
//! `TOCHeading`, `TableofFigures`, `ListParagraph`) gets a class
//! `stem-<id>`. Defaults that the docx side hard-codes into the
//! built-in styles are mirrored here so the HTML renders the same
//! visible look out of the box, and per-document overrides patch on
//! top — same precedence as docx.
//!
//! Native HTML elements with semantic mappings (`h1..h6`, `p`,
//! `figcaption`, `table > caption`, `a`) also pick up the same
//! rules via group selectors so authors don't have to add classes
//! everywhere just to get the Heading1 / Caption / Hyperlink look.

use std::fmt::Write;

use crate::style_props::LineHeight;

use super::ctx::StyleOverride;

/// Build the per-style CSS block for one document. Returns the full
/// list of CSS rules — concatenated by the caller into the
/// `<style>` element after the format's base rules.
pub fn document_style_css(overrides: &[StyleOverride]) -> String {
    let mut out = String::new();
    for (id, default_decls, selector_alias) in STYLE_TABLE {
        let override_decls = overrides
            .iter()
            .find(|o| o.id == *id)
            .map(decls_from_override)
            .unwrap_or_default();
        // Skip when both default + override are empty — no need to
        // emit a hollow rule.
        if default_decls.is_empty() && override_decls.is_empty() {
            continue;
        }
        let selector = if let Some(alias) = selector_alias {
            format!(".stem-doc {alias},.stem-doc .stem-{id}")
        } else {
            format!(".stem-doc .stem-{id}")
        };
        let _ = writeln!(
            out,
            "{selector}{{{default_decls}{override_decls}}}",
        );
    }
    out
}

/// The default decoration each named style starts with — copied
/// from the docx style registry so authors see the same look in
/// HTML as in Word without setting anything. Format:
/// `(id, default-css-decls, optional native-element selector)`.
///
/// Native-element selectors fold the style class into a group so a
/// bare `<h1>` picks up Heading1 defaults without needing the
/// `stem-Heading1` class. The class is still emitted by the heading
/// renderer so per-style overrides can hook either way.
const STYLE_TABLE: &[(&str, &str, Option<&str>)] = &[
    // Heading1 — Word's 2E74B5 (theme accent1) blue, 16pt.
    ("Heading1", "color:#2E74B5;font-size:16pt;font-weight:600;margin-top:12pt;margin-bottom:0;", Some("h1")),
    ("Heading2", "color:#2E74B5;font-size:13pt;font-weight:600;margin-top:2pt;margin-bottom:0;", Some("h2")),
    ("Heading3", "color:#2E74B5;font-size:12pt;font-weight:600;margin-top:2pt;margin-bottom:0;", Some("h3")),
    ("Heading4", "color:#2E74B5;font-size:11pt;font-weight:600;margin-top:2pt;margin-bottom:0;", Some("h4")),
    ("Heading5", "color:#2E74B5;font-size:11pt;font-weight:600;margin-top:2pt;margin-bottom:0;", Some("h5")),
    ("Heading6", "color:#2E74B5;font-size:11pt;font-weight:600;margin-top:2pt;margin-bottom:0;", Some("h6")),
    // Title — cover page heading. 18pt bold; centered by docx
    // default but we lean on the renderer to add the class so
    // <h1>-as-Title doesn't drag along the centering for body h1s.
    ("Title", "font-size:18pt;font-weight:bold;text-align:center;", None),
    // Caption — italic, mid-blue text, smaller, centered (Word's
    // built-in default). Both <figcaption> and <table>'s native
    // <caption> pick it up so authors don't have to add classes.
    ("Caption", "font-style:italic;color:#44546A;font-size:9pt;text-align:center;", Some("figcaption,table>caption")),
    // Hyperlink — blue + underlined; matches the docx Hyperlink
    // character style.
    ("Hyperlink", "color:#0563C1;text-decoration:underline;", Some("a")),
    // FootnoteReference — superscript marker.
    ("FootnoteReference", "vertical-align:super;font-size:smaller;", None),
    // TOC entries — left-indented per level, matching the docx
    // TOC1..9 ind:left = 220 * (level-1) dxa = 11pt per level.
    ("TOC1", "margin:0 0 5pt 0;", None),
    ("TOC2", "margin:0 0 5pt 0;padding-left:11pt;", None),
    ("TOC3", "margin:0 0 5pt 0;padding-left:22pt;", None),
    ("TOC4", "margin:0 0 5pt 0;padding-left:33pt;", None),
    ("TOC5", "margin:0 0 5pt 0;padding-left:44pt;", None),
    ("TOC6", "margin:0 0 5pt 0;padding-left:55pt;", None),
    ("TOC7", "margin:0 0 5pt 0;padding-left:66pt;", None),
    ("TOC8", "margin:0 0 5pt 0;padding-left:77pt;", None),
    ("TOC9", "margin:0 0 5pt 0;padding-left:88pt;", None),
    ("TOCHeading", "color:#2E74B5;font-size:16pt;font-weight:600;text-align:center;margin-top:12pt;margin-bottom:0;", None),
    ("TableofFigures", "", None),
    ("ListParagraph", "", None),
    ("Normal", "", Some("p")),
];

fn decls_from_override(o: &StyleOverride) -> String {
    let mut s = String::new();
    if let Some(v) = o.before_pt {
        let _ = write!(s, "margin-top:{}pt;", fmt_pt(v));
    }
    if let Some(v) = o.after_pt {
        let _ = write!(s, "margin-bottom:{}pt;", fmt_pt(v));
    }
    if let Some(lh) = o.line {
        match lh {
            LineHeight::Multiple(m) => {
                let _ = write!(s, "line-height:{};", fmt_num(m));
            }
            LineHeight::Points(p) => {
                let _ = write!(s, "line-height:{}pt;", fmt_pt(p));
            }
        }
    }
    if let Some(a) = &o.align {
        if let Some(v) = crate::style_props::map_align_css(a) {
            let _ = write!(s, "text-align:{};", v);
        }
    }
    if let Some(sz) = o.size_pt {
        let _ = write!(s, "font-size:{}pt;", fmt_pt(sz));
    }
    if let Some(c) = &o.color {
        let _ = write!(s, "color:#{};", c);
    }
    if let Some(c) = &o.bg {
        let _ = write!(s, "background:#{};", c);
    }
    if let Some(true) = o.bold {
        s.push_str("font-weight:bold;");
    }
    if o.bold == Some(false) {
        s.push_str("font-weight:normal;");
    }
    if let Some(true) = o.italic {
        s.push_str("font-style:italic;");
    }
    if o.italic == Some(false) {
        s.push_str("font-style:normal;");
    }
    if let Some(true) = o.strike {
        s.push_str("text-decoration:line-through;");
    }
    if let Some(u) = &o.underline {
        if u != "none" {
            s.push_str("text-decoration:underline;");
        }
    }
    if let Some(f) = &o.font {
        let _ = write!(s, "font-family:\"{}\";", f.replace('"', ""));
    }
    if o.border_top == Some(true) {
        s.push_str("border-top:1px solid currentColor;padding-top:4pt;");
    }
    s
}

fn fmt_pt(v: f64) -> String {
    fmt_num(v)
}

fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        // Trim trailing zeros from a fixed-precision form.
        let s = format!("{v:.3}");
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_table_emits_heading1_and_caption() {
        let css = document_style_css(&[]);
        assert!(css.contains(".stem-Heading1"));
        assert!(css.contains("#2E74B5"));
        // Caption pairs with figcaption + table>caption.
        assert!(css.contains("figcaption"));
        assert!(css.contains("table>caption"));
        // Hyperlink defaults to the docx blue + underline.
        assert!(css.contains("#0563C1"));
    }

    #[test]
    fn override_color_lands_after_default() {
        let css = document_style_css(&[StyleOverride {
            id: "Heading1".into(),
            color: Some("C0392B".into()),
            size_pt: Some(20.0),
            ..Default::default()
        }]);
        // The override comes after the default, so the cascade wins.
        let h1_block = css
            .split('\n')
            .find(|line| line.contains(".stem-Heading1"))
            .expect("heading1 rule");
        let default_pos = h1_block.find("#2E74B5").expect("default color");
        let override_pos = h1_block.find("#C0392B").expect("override color");
        assert!(default_pos < override_pos);
        let default_size = h1_block.find("font-size:16pt").expect("default size");
        let override_size = h1_block.find("font-size:20pt").expect("override size");
        assert!(default_size < override_size);
    }

    #[test]
    fn caption_align_left_overrides_center() {
        let css = document_style_css(&[StyleOverride {
            id: "Caption".into(),
            align: Some("left".into()),
            ..Default::default()
        }]);
        let cap_line = css
            .split('\n')
            .find(|line| line.contains(".stem-Caption"))
            .expect("caption rule");
        let center = cap_line.find("text-align:center").expect("default center");
        let left = cap_line.find("text-align:left").expect("override left");
        assert!(center < left);
    }
}
