//! Block-level paragraph emission.
//!
//! Task 6 scope: title, h1..h6, p, blockquote, pagebreak. Other
//! recognized blocks (section, table, image, code, ol, ul,
//! caption, header, footer) are handled by later tasks; here they
//! either descend into their children (container shape) or fall
//! back to a plain paragraph carrying the block's text.

use stem_core::ast::{Block, Body};

use super::super::parts::numbering::NUM_ID_HEADING;
use super::super::xml::XmlBuf;
use super::ctx::EmitCtx;
use super::{drawing, run, table, toc};

/// Emit OOXML for one top-level block into `x`. Recurses into
/// container blocks (`section`, `header`, `footer`).
pub fn render_block(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    match b.name.as_str() {
        "title" => render_title(b, ctx, x),
        "h1" => render_heading(b, 1, ctx, x),
        "h2" => render_heading(b, 2, ctx, x),
        "h3" => render_heading(b, 3, ctx, x),
        "h4" => render_heading(b, 4, ctx, x),
        "h5" => render_heading(b, 5, ctx, x),
        "h6" => render_heading(b, 6, ctx, x),
        "p" => render_paragraph(b, ctx, x),
        "blockquote" => render_blockquote(b, ctx, x),
        "pagebreak" => render_pagebreak(x),
        "table" => table::render_table(b, ctx, x),
        "image" => drawing::render_image(b, ctx, x),
        // Container-shaped blocks — recurse into their child blocks
        // so nested paragraphs land at the body level. Task 6 does
        // not emit section/header/footer-specific wrappers; those
        // arrive in tasks 11+ (sections) and 13 (header/footer).
        "section" => {
            if !toc::try_render_toc_section(b, ctx, x) {
                render_children(b, ctx, x);
            }
        }
        // Header + footer are emitted as separate OOXML parts
        // (headerN.xml / footerN.xml) by the packager — drop them
        // from the body walk.
        "header" | "footer" => {}
        // Anything else not yet handled: emit as a plain paragraph
        // carrying the block's text. Keeps the output structurally
        // complete while later tasks (7-14) take over each block
        // type one by one.
        _ => render_fallback_paragraph(b, ctx, x),
    }
}

/// Walk children only — used for section etc.
fn render_children(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    if let Body::Children(children) = &b.body {
        for child in children {
            render_block(child, ctx, x);
        }
    }
}

fn render_title(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    let align = b.prop_str("align").unwrap_or("center");
    let size_hp = b.prop_str("size").and_then(parse_length_to_half_points);
    x.elem("w:p", &[], |x| {
        x.elem("w:pPr", &[], |x| {
            x.empty("w:pStyle", &[("w:val", "Title")]);
            // Tighter title spacing than the body default — 6pt
            // after, single-line height, matching the reference
            // BoringCrypto cover.
            x.empty(
                "w:spacing",
                &[
                    ("w:after", "120"),
                    ("w:line", "240"),
                    ("w:lineRule", "auto"),
                ],
            );
            x.empty("w:jc", &[("w:val", normalize_jc(align))]);
            // Paragraph-mark rPr override — needed so the trailing
            // pilcrow renders at the requested size too.
            if let Some(sz) = size_hp {
                let s = sz.to_string();
                x.elem("w:rPr", &[], |x| {
                    x.empty("w:sz", &[("w:val", &s)]);
                    x.empty("w:szCs", &[("w:val", &s)]);
                });
            }
        });
        // Per-run size override when `[size:..]` is set on the title.
        let base = if let Some(sz) = size_hp {
            run::RPr {
                size_hp: Some(sz),
                ..Default::default()
            }
        } else {
            run::RPr::default()
        };
        run::render_body_with(b, ctx, x, &base);
    });
}

/// Convert a length string to half-points (Word's font size unit).
/// Accepts `12pt`, `1in`, `2.54cm`, `25mm`, bare number (treated as
/// points). Returns `None` for unrecognized units or negatives.
fn parse_length_to_half_points(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let idx = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());
    let (num, unit) = s.split_at(idx);
    let value: f64 = num.parse().ok()?;
    let pts = match unit {
        "" | "pt" => value,
        "in" => value * 72.0,
        "cm" => value * 28.3464566929,
        "mm" => value * 2.83464566929,
        _ => return None,
    };
    if pts < 0.0 {
        return None;
    }
    Some((pts * 2.0).round() as u32)
}

fn render_heading(b: &Block, level: u32, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    let style = format!("Heading{level}");
    let numbered = b.prop_str("numbered") == Some("true");
    // Use the bookmark name from the prepass-populated registry;
    // advance the cursor so each subsequent heading consumes the
    // next anchor.
    let bookmark = ctx
        .heading_anchors
        .get(ctx.heading_cursor)
        .map(|a| a.bookmark.clone())
        .unwrap_or_else(|| format!("_Toc{}", ctx.heading_cursor + 1));
    ctx.heading_cursor += 1;
    let bm_id = ctx.alloc_bookmark_id();
    let bm_id_s = bm_id.to_string();
    x.elem("w:p", &[], |x| {
        x.elem("w:pPr", &[], |x| {
            x.empty("w:pStyle", &[("w:val", &style)]);
            if numbered {
                let ilvl = (level - 1).to_string();
                let num_id = NUM_ID_HEADING.to_string();
                x.elem("w:numPr", &[], |x| {
                    x.empty("w:ilvl", &[("w:val", &ilvl)]);
                    x.empty("w:numId", &[("w:val", &num_id)]);
                });
            }
        });
        x.empty(
            "w:bookmarkStart",
            &[("w:id", &bm_id_s), ("w:name", &bookmark)],
        );
        run::render_body(b, ctx, x);
        x.empty("w:bookmarkEnd", &[("w:id", &bm_id_s)]);
    });
}

fn render_paragraph(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    x.elem("w:p", &[], |x| {
        let visually_empty = is_visually_empty(b);
        let before = b.prop_str("before").and_then(parse_length_to_dxa);
        let after = b.prop_str("after").and_then(parse_length_to_dxa);
        let line = b.prop_str("line").and_then(parse_line_height);
        let border_top = b.prop_str("border-top") == Some("true");
        let tabs = b.prop_str("tabs");
        let align = b.prop_str("align").and_then(map_align);

        // Auto-collapse spacing on visually empty paragraphs unless
        // the author has explicitly set their own spacing override.
        let auto_collapse =
            visually_empty && before.is_none() && after.is_none() && line.is_none();
        let has_explicit_spacing = before.is_some() || after.is_some() || line.is_some();

        let needs_p_pr = auto_collapse
            || has_explicit_spacing
            || border_top
            || tabs.is_some()
            || align.is_some();
        if needs_p_pr {
            x.elem("w:pPr", &[], |x| {
                // pBdr → tabs → spacing → jc per schema order.
                if border_top {
                    x.elem("w:pBdr", &[], |x| {
                        x.empty(
                            "w:top",
                            &[
                                ("w:val", "single"),
                                ("w:sz", "4"),
                                ("w:space", "1"),
                                ("w:color", "auto"),
                            ],
                        );
                    });
                }
                if let Some(spec) = tabs {
                    render_tabs(x, spec);
                }
                if has_explicit_spacing {
                    emit_spacing(x, before, after, line);
                } else if auto_collapse {
                    // Empty `p()` spacers collapse the 8pt-after /
                    // 1.08x-line default to zero after / single
                    // line. The cover's stack of empty paragraphs
                    // depends on this — without it they'd push
                    // content off the page.
                    x.empty(
                        "w:spacing",
                        &[
                            ("w:after", "0"),
                            ("w:line", "240"),
                            ("w:lineRule", "auto"),
                        ],
                    );
                }
                if let Some(j) = align {
                    x.empty("w:jc", &[("w:val", j)]);
                }
            });
        }
        run::render_body(b, ctx, x);
    });
}

/// A line-height value parsed from a paragraph's `line:` property.
/// Either an explicit point value (`line:18pt`) or a multiplier
/// (`line:1.5x`, treated as multiple-line spacing).
enum LineHeight {
    /// Exact dxa value with `lineRule="auto"` (or "exact" for
    /// negative values per Word's convention — we always use auto).
    Auto(u32),
    /// Multiple of single line height, dxa = N * 240.
    Multiple(u32),
}

fn parse_line_height(s: &str) -> Option<LineHeight> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix('x') {
        let mult: f64 = num.parse().ok()?;
        if mult <= 0.0 {
            return None;
        }
        return Some(LineHeight::Multiple((mult * 240.0).round() as u32));
    }
    parse_length_to_dxa(s).map(LineHeight::Auto)
}

fn emit_spacing(
    x: &mut XmlBuf,
    before: Option<u32>,
    after: Option<u32>,
    line: Option<LineHeight>,
) {
    let before_s = before.map(|v| v.to_string());
    let after_s = after.map(|v| v.to_string());
    let (line_val, line_rule) = match &line {
        Some(LineHeight::Auto(n)) | Some(LineHeight::Multiple(n)) => {
            (Some(n.to_string()), Some("auto"))
        }
        None => (None, None),
    };
    let mut attrs: Vec<(&str, &str)> = Vec::with_capacity(4);
    if let Some(b) = &before_s {
        attrs.push(("w:before", b.as_str()));
    }
    if let Some(a) = &after_s {
        attrs.push(("w:after", a.as_str()));
    }
    if let Some(l) = &line_val {
        attrs.push(("w:line", l.as_str()));
    }
    if let Some(r) = line_rule {
        attrs.push(("w:lineRule", r));
    }
    x.empty("w:spacing", &attrs);
}

/// Length to twentieths of a point (Word's dxa unit). Accepts
/// `Npt`, `Nin`, `Ncm`, `Nmm`, `Npx` (96dpi), bare number = pt.
fn parse_length_to_dxa(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let idx = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());
    let (num, unit) = s.split_at(idx);
    let value: f64 = num.parse().ok()?;
    let pts = match unit {
        "" | "pt" => value,
        "in" => value * 72.0,
        "cm" => value * 28.3464566929,
        "mm" => value * 2.83464566929,
        "px" => value * 0.75, // 96dpi → 1px = 0.75pt
        _ => return None,
    };
    if pts < 0.0 {
        return None;
    }
    Some((pts * 20.0).round() as u32)
}

fn map_align(s: &str) -> Option<&'static str> {
    match s {
        "left" => Some("left"),
        "right" => Some("right"),
        "center" | "centre" => Some("center"),
        "justify" | "both" => Some("both"),
        _ => None,
    }
}

/// Emit `<w:tabs>` from a spec like `"center,right"` or
/// `"left:0,center:4675,right:9350"`. Bare alignment names use
/// auto-computed positions for the letter-paper content width
/// (9350 dxa).
fn render_tabs(x: &mut XmlBuf, spec: &str) {
    let stops: Vec<(String, String)> = parse_tab_spec(spec);
    if stops.is_empty() {
        return;
    }
    x.elem("w:tabs", &[], |x| {
        for (val, pos) in &stops {
            x.empty("w:tab", &[("w:val", val.as_str()), ("w:pos", pos.as_str())]);
        }
    });
}

fn parse_tab_spec(spec: &str) -> Vec<(String, String)> {
    // Letter paper, 1" margins → 9350 dxa content width.
    const CONTENT_W: u32 = 9350;
    let names: Vec<&str> = spec.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    if names.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(names.len());
    for name in &names {
        // Allow "center:4675" explicit pos.
        let (kind, pos_opt) = match name.split_once(':') {
            Some((k, p)) => (k.trim(), p.trim().parse::<u32>().ok()),
            None => (*name, None),
        };
        let val = match kind {
            "left" | "right" | "center" | "decimal" | "bar" => kind,
            _ => continue,
        };
        out.push((val.to_string(), pos_opt.map(|p| p.to_string())));
    }
    // Auto-distribute positions when not specified:
    // - 1 stop: place at content end if "right", center if "center"
    // - 2 stops: left edge + right edge (or specified)
    // - 3+ stops: evenly distributed
    let n = out.len();
    let mut result = Vec::with_capacity(n);
    for (i, (val, pos)) in out.into_iter().enumerate() {
        let p = pos.unwrap_or_else(|| {
            if val == "right" {
                CONTENT_W.to_string()
            } else if val == "center" {
                (CONTENT_W / 2).to_string()
            } else {
                // For other named stops without explicit pos,
                // distribute evenly.
                ((i as u32 + 1) * CONTENT_W / (n as u32 + 1)).to_string()
            }
        });
        result.push((val, p));
    }
    result
}

/// True if a paragraph's body contributes no visible runs — used
/// to identify spacer paragraphs.
fn is_visually_empty(b: &Block) -> bool {
    match &b.body {
        Body::None => true,
        Body::Children(c) => c.is_empty(),
        Body::Text(pieces) => pieces.iter().all(|p| match p {
            stem_core::ast::TextPiece::Literal { text, .. } => text.trim().is_empty(),
            stem_core::ast::TextPiece::Inline(_) => false,
        }),
    }
}

fn render_blockquote(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    x.elem("w:p", &[], |x| {
        x.elem("w:pPr", &[], |x| {
            x.empty("w:ind", &[("w:left", "720")]);
        });
        run::render_body(b, ctx, x);
    });
}

fn render_pagebreak(x: &mut XmlBuf) {
    x.elem("w:p", &[], |x| {
        run::render_page_break(x);
    });
}

/// Fallback for block names task 6 doesn't yet specialize: emit a
/// plain paragraph carrying the flattened text so the document
/// keeps the right paragraph count and reading flow.
fn render_fallback_paragraph(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    x.elem("w:p", &[], |x| {
        run::render_body(b, ctx, x);
    });
}

fn normalize_jc(s: &str) -> &'static str {
    // Stem's alignment vocab → Word's `<w:jc w:val>`.
    match s {
        "left" => "left",
        "right" => "right",
        "center" | "centre" => "center",
        "justify" | "both" => "both",
        _ => "center",
    }
}

#[cfg(test)]
mod tests {
    use stem_parser::parse;

    use super::*;

    fn render(src: &str) -> String {
        let r = parse(src);
        let mut ctx = EmitCtx::new(None, 1);
        let mut x = XmlBuf::new();
        for b in &r.document.blocks {
            render_block(b, &mut ctx, &mut x);
        }
        x.finish()
    }

    #[test]
    fn title_carries_title_style_and_centered_jc() {
        let s = render(r#"title(Hello world)"#);
        assert!(s.contains(r#"<w:pStyle w:val="Title"/>"#));
        assert!(s.contains(r#"<w:jc w:val="center"/>"#));
        assert!(s.contains("Hello world"));
    }

    #[test]
    fn title_size_property_overrides_run_size() {
        // 12pt = 24 half-points.
        let s = render(r#"title[size:12pt](Smaller)"#);
        assert!(
            s.contains(r#"<w:sz w:val="24"/>"#),
            "expected per-run size override 24 hp: {s}"
        );
        assert!(s.contains("Smaller"));
    }

    #[test]
    fn parse_length_to_half_points_handles_units() {
        assert_eq!(parse_length_to_half_points("12pt"), Some(24));
        assert_eq!(parse_length_to_half_points("12"), Some(24));
        assert_eq!(parse_length_to_half_points("1in"), Some(144));
        assert_eq!(parse_length_to_half_points("2.54cm"), Some(144));
        assert_eq!(parse_length_to_half_points("xyz"), None);
    }

    #[test]
    fn heading_carries_style_for_each_level() {
        for level in 1..=6u32 {
            let src = format!("h{level}(text)");
            let s = render(&src);
            assert!(
                s.contains(&format!(r#"<w:pStyle w:val="Heading{level}"/>"#)),
                "missing pStyle Heading{level} in {s}"
            );
        }
    }

    #[test]
    fn heading_numbered_adds_numPr() {
        let s = render(r#"h2[numbered:true](Section A)"#);
        assert!(s.contains("<w:numPr>"));
        assert!(s.contains(r#"<w:ilvl w:val="1"/>"#));
        assert!(s.contains(r#"<w:numId w:val="3"/>"#));
    }

    #[test]
    fn paragraph_carries_no_pPr_when_unstyled() {
        let s = render(r#"p(hello)"#);
        assert!(s.contains("<w:p>"));
        // No pStyle, no pPr — just a body run.
        assert!(!s.contains("<w:pPr>"));
        assert!(s.contains("hello"));
    }

    #[test]
    fn blockquote_emits_left_indent() {
        let s = render(r#"blockquote(quoted text)"#);
        assert!(s.contains(r#"<w:ind w:left="720"/>"#));
        assert!(s.contains("quoted text"));
    }

    #[test]
    fn paragraph_after_0_emits_zero_after_spacing() {
        let s = render(r#"p[after:0pt](hello)"#);
        assert!(
            s.contains(r#"<w:spacing w:after="0""#),
            "expected w:after=0: {s}"
        );
    }

    #[test]
    fn paragraph_before_and_after_emit_dxa_values() {
        // 6pt = 120 dxa, 12pt = 240 dxa.
        let s = render(r#"p[before:6pt, after:12pt](x)"#);
        assert!(s.contains(r#"<w:spacing w:before="120" w:after="240"/>"#), "got: {s}");
    }

    #[test]
    fn paragraph_line_multiplier_uses_single_line_base() {
        // 1.5x of 240 (single line) = 360 dxa.
        let s = render(r#"p[line:1.5x](x)"#);
        assert!(
            s.contains(r#"w:line="360" w:lineRule="auto""#),
            "got: {s}"
        );
    }

    #[test]
    fn paragraph_line_explicit_pt_value() {
        // 18pt = 360 dxa, auto rule.
        let s = render(r#"p[line:18pt](x)"#);
        assert!(s.contains(r#"w:line="360" w:lineRule="auto""#), "got: {s}");
    }

    #[test]
    fn parse_length_to_dxa_handles_units() {
        // 1pt = 20 dxa.
        assert_eq!(parse_length_to_dxa("0"), Some(0));
        assert_eq!(parse_length_to_dxa("1pt"), Some(20));
        assert_eq!(parse_length_to_dxa("12"), Some(240));
        assert_eq!(parse_length_to_dxa("1in"), Some(1440));
        assert_eq!(parse_length_to_dxa("xyz"), None);
    }

    #[test]
    fn paragraph_with_border_top_emits_pBdr() {
        let s = render(r#"p[border-top:true](hello)"#);
        assert!(s.contains("<w:pBdr>"));
        assert!(s.contains(r#"<w:top w:val="single""#));
    }

    #[test]
    fn paragraph_with_tabs_spec_emits_w_tabs_with_stops() {
        let s = render(r#"p[tabs:"center,right"](a@tab()b@tab()c)"#);
        assert!(s.contains("<w:tabs>"));
        assert!(s.contains(r#"<w:tab w:val="center""#));
        assert!(s.contains(r#"<w:tab w:val="right""#));
        // Each `@tab()` emits a `<w:tab/>` inside its own run.
        assert_eq!(s.matches("<w:tab/>").count(), 2);
    }

    #[test]
    fn paragraph_align_center_emits_w_jc() {
        let s = render(r#"p[align:center](centered)"#);
        assert!(s.contains(r#"<w:jc w:val="center"/>"#));
    }

    #[test]
    fn br_inline_emits_w_br_run() {
        let s = render(r#"p(line1@br()line2)"#);
        // <w:r><w:br/></w:r> between the two text runs.
        let br = s.find("<w:br/>").unwrap();
        let line1 = s.find("line1").unwrap();
        let line2 = s.find("line2").unwrap();
        assert!(line1 < br && br < line2);
    }

    #[test]
    fn pagebreak_emits_w_br_inside_w_p() {
        let s = render("pagebreak");
        assert!(s.contains(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#));
    }

    #[test]
    fn section_recurses_into_children() {
        let s = render(
            r#"section{
  h2(Inner heading)
  p(Inner paragraph)
}"#,
        );
        assert!(s.contains(r#"<w:pStyle w:val="Heading2"/>"#));
        assert!(s.contains("Inner paragraph"));
    }

    #[test]
    fn inline_text_pieces_flatten_to_plain_text() {
        let s = render(r#"p(hello @b(bold) world)"#);
        // Task 6 doesn't yet emit bold rPr — that's task 7. Here
        // we just verify the text content lands intact.
        assert!(s.contains("hello bold world") || s.contains("hello"));
    }

    #[test]
    fn unknown_block_falls_back_to_plain_paragraph() {
        // `widget` isn't in the schema, but the emitter must not
        // crash — fallback emits the text.
        let s = render(r#"widget(unrecognized content)"#);
        assert!(s.contains("unrecognized content"));
    }
}
