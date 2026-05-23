//! Runs — `<w:r>` + `<w:rPr>` emission from text pieces.
//!
//! Walks a block's `Body::Text` piece list and emits one `<w:r>`
//! per piece, stacking run-property overrides from nested inline
//! elements. The current exporter's "flatten everything except
//! bold/italic" behavior is replaced here with a real recursive
//! traversal so a paragraph like
//!
//!   `p(plain @text[weight:bold](bold @text[style:italic](both)) tail)`
//!
//! produces four runs: plain, bold, bold+italic, plain.
//!
//! Inline elements task 7 specializes:
//! - `@text` — `weight`, `style`, `decoration`, `color`, `bg`
//! - `@code` — monospace (Courier New)
//!
//! Inline elements task 7 defers (rendered as plain runs with the
//! inherited rPr, no extra styling):
//! - `@link` — task 11 wires actual hyperlinks + Hyperlink style
//! - `@footnote` — task 14 wires the footnotes part
//! - `@page-number`, `@total-pages` — task 10 wires PAGE/NUMPAGES
//!   fields
//!
//! `<w:rPr>` children land in canonical order:
//!   rFonts → b → bCs → i → iCs → strike → color → sz → szCs →
//!   highlight → u → vertAlign

use stem_core::ast::{Block, Body, TextPiece};

use super::super::xml::XmlBuf;
use super::ctx::EmitCtx;
use super::{field, hyperlink};

/// Run properties. `Option` fields layer over inherited values
/// from a parent inline; `bool` fields turn a property on
/// permanently.
#[derive(Default, Clone, Debug)]
pub struct RPr {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    /// Underline value — `"single"`, `"double"`, `"none"`, etc.
    pub underline: Option<String>,
    /// Foreground color as 6-hex RGB (no leading `#`).
    pub color: Option<String>,
    /// Background highlight name (`"yellow"`, `"green"`, etc.) or
    /// `"auto"`. We map arbitrary RGB to the nearest highlight
    /// when the source gives a hex — OOXML's `<w:highlight>` is
    /// name-only, so a true RGB background falls back to a
    /// shading element on the run.
    pub highlight: Option<String>,
    /// Run-level shading fill — used when the source supplies a
    /// hex `bg` color that can't be expressed via `<w:highlight>`.
    pub shading_fill: Option<String>,
    /// Half-points (Word's units).
    pub size_hp: Option<u32>,
    /// Font family (e.g. "Courier New" for `@code`). Applied to
    /// `ascii`/`hAnsi` only — East-Asian fallbacks stay with the
    /// document defaults.
    pub font_face: Option<String>,
    /// Character style — applied via `<w:rStyle w:val="..."/>`.
    /// Used by the Hyperlink-style fallback when task 11's full
    /// hyperlink emission isn't reached.
    pub r_style: Option<String>,
}

impl RPr {
    pub fn is_empty(&self) -> bool {
        !self.bold
            && !self.italic
            && !self.strike
            && self.underline.is_none()
            && self.color.is_none()
            && self.highlight.is_none()
            && self.shading_fill.is_none()
            && self.size_hp.is_none()
            && self.font_face.is_none()
            && self.r_style.is_none()
    }

    /// Layer `other` on top of `self`. Boolean flags OR together;
    /// `Option` fields prefer `other` when set, otherwise inherit
    /// from `self`.
    pub fn merged(&self, other: &RPr) -> RPr {
        RPr {
            bold: self.bold || other.bold,
            italic: self.italic || other.italic,
            strike: self.strike || other.strike,
            underline: other.underline.clone().or_else(|| self.underline.clone()),
            color: other.color.clone().or_else(|| self.color.clone()),
            highlight: other.highlight.clone().or_else(|| self.highlight.clone()),
            shading_fill: other
                .shading_fill
                .clone()
                .or_else(|| self.shading_fill.clone()),
            size_hp: other.size_hp.or(self.size_hp),
            font_face: other.font_face.clone().or_else(|| self.font_face.clone()),
            r_style: other.r_style.clone().or_else(|| self.r_style.clone()),
        }
    }

    /// Emit `<w:rPr>` with children in canonical schema order.
    /// No-op if `is_empty()`.
    pub fn render(&self, x: &mut XmlBuf) {
        if self.is_empty() {
            return;
        }
        x.elem("w:rPr", &[], |x| {
            if let Some(style_id) = &self.r_style {
                x.empty("w:rStyle", &[("w:val", style_id.as_str())]);
            }
            if let Some(face) = &self.font_face {
                x.empty(
                    "w:rFonts",
                    &[("w:ascii", face.as_str()), ("w:hAnsi", face.as_str())],
                );
            }
            if self.bold {
                x.empty("w:b", &[]);
                x.empty("w:bCs", &[]);
            }
            if self.italic {
                x.empty("w:i", &[]);
                x.empty("w:iCs", &[]);
            }
            if self.strike {
                x.empty("w:strike", &[]);
            }
            if let Some(c) = &self.color {
                x.empty("w:color", &[("w:val", c.as_str())]);
            }
            if let Some(sz) = self.size_hp {
                let s = sz.to_string();
                x.empty("w:sz", &[("w:val", &s)]);
                x.empty("w:szCs", &[("w:val", &s)]);
            }
            if let Some(name) = &self.highlight {
                x.empty("w:highlight", &[("w:val", name.as_str())]);
            }
            if let Some(fill) = &self.shading_fill {
                x.empty(
                    "w:shd",
                    &[
                        ("w:val", "clear"),
                        ("w:color", "auto"),
                        ("w:fill", fill.as_str()),
                    ],
                );
            }
            if let Some(u) = &self.underline {
                x.empty("w:u", &[("w:val", u.as_str())]);
            }
        });
    }
}

/// Append every run that belongs to `b`'s body. Inline elements
/// stack their rPr on top of `base`. The mutable `ctx` lets
/// inline-emit hand-offs (hyperlinks, footnotes, ...) register
/// the rels/parts they need.
pub fn render_body(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    render_body_with(b, ctx, x, &RPr::default());
}

/// Same as [`render_body`] but starts from the supplied base rPr —
/// used when a paragraph itself imposes formatting on its content
/// (e.g. blockquote's italic in some style sets).
pub fn render_body_with(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf, base: &RPr) {
    if let Body::Text(pieces) = &b.body {
        for piece in pieces {
            render_piece(piece, ctx, x, base);
        }
    }
}

fn render_piece(piece: &TextPiece, ctx: &mut EmitCtx, x: &mut XmlBuf, parent: &RPr) {
    match piece {
        TextPiece::Literal { text, .. } => {
            if !text.is_empty() {
                emit_run(text, parent, x);
            }
        }
        TextPiece::Inline(inline) => render_inline(inline, ctx, x, parent),
    }
}

fn render_inline(inline: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf, parent: &RPr) {
    match inline.name.as_str() {
        // Specialized rPr extraction.
        "text" => {
            let layered = parent.merged(&rpr_from_text(inline));
            render_inner(inline, ctx, x, &layered);
        }
        "code" => {
            let layered = parent.merged(&rpr_code());
            render_inner(inline, ctx, x, &layered);
        }
        "link" => hyperlink::render_link(inline, ctx, parent, x),
        "footnote" => {
            // Capture the body as the footnote content; emit a
            // superscript footnote reference at the current run
            // position.
            let text = flatten_inline_text(inline);
            let id = ctx.add_footnote(text);
            emit_footnote_ref(id, x);
        }
        "page-number" => field::render_page(x),
        "total-pages" => field::render_num_pages(x),
        // Other inline elements — emit their text recursively
        // with no extra styling. Future tasks specialize as
        // needed.
        _ => render_inner(inline, ctx, x, parent),
    }
}

/// Recurse into an inline's body so nested inlines keep stacking.
fn render_inner(inline: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf, current: &RPr) {
    match &inline.body {
        Body::None => {}
        Body::Text(pieces) => {
            for piece in pieces {
                render_piece(piece, ctx, x, current);
            }
        }
        Body::Children(blocks) => {
            // Inline elements rarely have children-body, but if
            // they do, walk their text via the same dispatch.
            for child in blocks {
                render_body_with(child, ctx, x, current);
            }
        }
    }
}

fn rpr_from_text(b: &Block) -> RPr {
    let mut r = RPr::default();
    match b.prop_str("weight") {
        Some("bold") => r.bold = true,
        Some("light") => {} // no half-bold/light rPr in our scope
        _ => {}
    }
    if b.prop_str("style") == Some("italic") || b.prop_str("style") == Some("oblique") {
        r.italic = true;
    }
    match b.prop_str("decoration") {
        Some("underline") => r.underline = Some("single".into()),
        Some("strike") => r.strike = true,
        _ => {}
    }
    if let Some(c) = b.prop_str("color") {
        r.color = normalize_color(c);
    }
    if let Some(bg) = b.prop_str("bg") {
        if let Some(hex) = normalize_color(bg) {
            // No exact name match → shading fill.
            r.shading_fill = Some(hex);
        }
    }
    r
}

fn rpr_code() -> RPr {
    RPr {
        font_face: Some("Courier New".into()),
        ..Default::default()
    }
}

/// rPr for a `@link` content run when task 11's hyperlink wiring
/// hasn't yet replaced this fallback. Applies Word's Hyperlink
/// character style so the visual (blue + underline) still lands.
fn rpr_hyperlink() -> RPr {
    RPr {
        r_style: Some("Hyperlink".into()),
        ..Default::default()
    }
}

/// Accept either a `#RRGGBB`, `RRGGBB`, or a CSS-style named color.
/// Returns canonical `RRGGBB` uppercase (Word's expected form).
fn normalize_color(s: &str) -> Option<String> {
    let t = s.trim().trim_start_matches('#');
    if t.len() == 6 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(t.to_uppercase());
    }
    if t.len() == 3 && t.chars().all(|c| c.is_ascii_hexdigit()) {
        // Expand "abc" to "AABBCC".
        let mut out = String::with_capacity(6);
        for c in t.chars() {
            let u = c.to_ascii_uppercase();
            out.push(u);
            out.push(u);
        }
        return Some(out);
    }
    None
}

/// Emit one complete `<w:r>...</w:r>` carrying `text` and an
/// optional `<w:rPr>` derived from `rpr`.
fn emit_run(text: &str, rpr: &RPr, x: &mut XmlBuf) {
    x.elem("w:r", &[], |x| {
        rpr.render(x);
        x.elem_text("w:t", &[], text, true);
    });
}

/// Emit a `<w:footnoteReference w:id="N"/>` run with the
/// FootnoteReference character style. Lands at the cursor so the
/// surrounding text reads "see prior text¹".
fn emit_footnote_ref(id: u32, x: &mut XmlBuf) {
    let id_s = id.to_string();
    x.elem("w:r", &[], |x| {
        x.elem("w:rPr", &[], |x| {
            x.empty("w:rStyle", &[("w:val", "FootnoteReference")]);
        });
        x.empty("w:footnoteReference", &[("w:id", &id_s)]);
    });
}

fn flatten_inline_text(b: &Block) -> String {
    let mut out = String::new();
    if let Body::Text(pieces) = &b.body {
        for p in pieces {
            match p {
                TextPiece::Literal { text, .. } => out.push_str(text),
                TextPiece::Inline(inner) => out.push_str(&flatten_inline_text(inner)),
            }
        }
    }
    out
}

/// Emit `<w:r><w:br w:type="page"/></w:r>`. Used by pagebreak.
pub fn render_page_break(x: &mut XmlBuf) {
    x.elem("w:r", &[], |x| {
        x.empty("w:br", &[("w:type", "page")]);
    });
}

#[cfg(test)]
mod tests {
    use stem_parser::parse;

    use super::*;

    fn first_block(src: &str) -> Block {
        let r = parse(src);
        r.document.blocks.first().unwrap().clone()
    }

    fn render_runs(src: &str) -> String {
        let b = first_block(src);
        let mut ctx = EmitCtx::new(None, 1);
        let mut x = XmlBuf::new();
        render_body(&b, &mut ctx, &mut x);
        x.finish()
    }

    #[test]
    fn empty_body_renders_nothing() {
        assert_eq!(render_runs("p()"), "");
    }

    #[test]
    fn plain_literal_emits_single_run_with_preserve() {
        let s = render_runs("p(hello)");
        assert_eq!(s, r#"<w:r><w:t xml:space="preserve">hello</w:t></w:r>"#);
    }

    #[test]
    fn text_weight_bold_emits_w_b_and_bCs() {
        let s = render_runs(r#"p(plain @text[weight:bold](bold) tail)"#);
        // Three runs: "plain ", "bold", " tail".
        assert_eq!(s.matches("<w:r>").count(), 3);
        // Middle run has <w:b/> and <w:bCs/>.
        assert!(s.contains("<w:rPr><w:b/><w:bCs/></w:rPr>"));
        // Plain runs have no rPr.
        assert!(s.starts_with(r#"<w:r><w:t xml:space="preserve">plain </w:t></w:r>"#));
    }

    #[test]
    fn text_style_italic_emits_w_i_and_iCs() {
        let s = render_runs(r#"p(@text[style:italic](em))"#);
        assert!(s.contains("<w:i/>"));
        assert!(s.contains("<w:iCs/>"));
    }

    #[test]
    fn text_decoration_strike_emits_w_strike() {
        let s = render_runs(r#"p(@text[decoration:strike](del))"#);
        assert!(s.contains("<w:strike/>"));
    }

    #[test]
    fn text_decoration_underline_emits_w_u_single() {
        let s = render_runs(r#"p(@text[decoration:underline](u))"#);
        assert!(s.contains(r#"<w:u w:val="single"/>"#));
    }

    #[test]
    fn text_color_emits_w_color_uppercase() {
        let s = render_runs(r##"p(@text[color:"#ff0000"](red))"##);
        assert!(s.contains(r#"<w:color w:val="FF0000"/>"#), "got: {s}");
    }

    #[test]
    fn code_emits_courier_new_rfonts() {
        let s = render_runs(r#"p(see @code(fn()) here)"#);
        assert!(
            s.contains(r#"<w:rFonts w:ascii="Courier New" w:hAnsi="Courier New"/>"#),
            "got: {s}"
        );
    }

    #[test]
    fn nested_inlines_stack_rpr() {
        let s = render_runs(r#"p(@text[weight:bold](b @text[style:italic](both)))"#);
        // Two text runs: "b " (bold) and "both" (bold+italic).
        // The bold-only run has b/bCs but no i/iCs.
        let b_run = s.find(r#"<w:t xml:space="preserve">b </w:t>"#).unwrap();
        let b_run_start = s[..b_run].rfind("<w:r>").unwrap();
        let b_block = &s[b_run_start..b_run];
        assert!(b_block.contains("<w:b/>"));
        assert!(!b_block.contains("<w:i/>"));
        // The "both" run has both bold and italic.
        let both = s.find(r#"<w:t xml:space="preserve">both</w:t>"#).unwrap();
        let both_start = s[..both].rfind("<w:r>").unwrap();
        let both_block = &s[both_start..both];
        assert!(both_block.contains("<w:b/>") && both_block.contains("<w:i/>"));
    }

    #[test]
    fn link_emits_w_hyperlink_with_hyperlink_style() {
        let s = render_runs(r#"p(visit @link[to:"https://x"](this site) please)"#);
        assert!(s.contains("<w:hyperlink "));
        assert!(s.contains(r#"<w:rStyle w:val="Hyperlink"/>"#));
        assert!(s.contains("this site"));
    }

    #[test]
    fn footnote_inline_emits_reference_run_and_captures_text() {
        let b = first_block(r#"p(see @footnote(foo body) end)"#);
        let mut ctx = EmitCtx::new(None, 1);
        let mut x = XmlBuf::new();
        render_body(&b, &mut ctx, &mut x);
        let s = x.finish();
        // Visible body keeps "see " and " end" — the marker
        // doesn't contain the footnote text.
        assert!(s.contains("see "));
        assert!(s.contains(" end"));
        // <w:footnoteReference w:id="1"/> + FootnoteReference style.
        assert!(s.contains(r#"<w:footnoteReference w:id="1"/>"#));
        assert!(s.contains(r#"<w:rStyle w:val="FootnoteReference"/>"#));
        // Captured footnote content is "foo body".
        assert_eq!(ctx.footnotes.len(), 1);
        assert_eq!(ctx.footnotes[0].id, 1);
        assert_eq!(ctx.footnotes[0].text, "foo body");
    }

    #[test]
    fn page_number_inline_emits_PAGE_field() {
        let s = render_runs(r#"p(Page @page-number() of @total-pages())"#);
        assert!(s.contains(r#"w:instr=" PAGE   \* MERGEFORMAT ""#));
        assert!(s.contains(r#"w:instr=" NUMPAGES   \* MERGEFORMAT ""#));
    }

    #[test]
    fn rpr_child_order_is_canonical() {
        // Cover every slot at once and assert ordering.
        let r = RPr {
            r_style: Some("Hyperlink".into()),
            font_face: Some("Courier New".into()),
            bold: true,
            italic: true,
            strike: true,
            color: Some("0563C1".into()),
            size_hp: Some(22),
            highlight: Some("yellow".into()),
            shading_fill: Some("FFFF00".into()),
            underline: Some("single".into()),
        };
        let mut x = XmlBuf::new();
        r.render(&mut x);
        let s = x.finish();
        // rStyle → rFonts → b/bCs → i/iCs → strike → color →
        // sz/szCs → highlight → shd → u
        let positions = [
            ("<w:rStyle", s.find("<w:rStyle").unwrap()),
            ("<w:rFonts", s.find("<w:rFonts").unwrap()),
            ("<w:b/>", s.find("<w:b/>").unwrap()),
            ("<w:i/>", s.find("<w:i/>").unwrap()),
            ("<w:strike/>", s.find("<w:strike/>").unwrap()),
            ("<w:color", s.find("<w:color").unwrap()),
            ("<w:sz ", s.find("<w:sz ").unwrap()),
            ("<w:highlight", s.find("<w:highlight").unwrap()),
            ("<w:shd ", s.find("<w:shd ").unwrap()),
            ("<w:u ", s.find("<w:u ").unwrap()),
        ];
        for win in positions.windows(2) {
            assert!(win[0].1 < win[1].1, "{} not before {}", win[0].0, win[1].0);
        }
    }

    #[test]
    fn page_break_emits_w_br() {
        let mut x = XmlBuf::new();
        render_page_break(&mut x);
        assert_eq!(x.finish(), r#"<w:r><w:br w:type="page"/></w:r>"#);
    }
}
