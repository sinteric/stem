//! TOC / List-of-Tables / List-of-Figures emission.
//!
//! Three reserved `section[id:...]` IDs:
//! - `toc` — table of contents.
//! - `list-of-tables` (or `lot`) — table-caption index.
//! - `list-of-figures` (or `lof`) — figure-caption index.
//!
//! Each emits a styled heading paragraph followed by:
//! - the field's `begin` + `instrText` + `separate` runs in one
//!   paragraph,
//! - one pre-populated entry paragraph per anchor (styled TOC<n>
//!   or TableofFigures) containing a hyperlink to the anchor and
//!   a PAGEREF for the page number,
//! - the field's `end` marker on the last entry's paragraph.
//!
//! We bypass the SDT (structured document tag) wrapper Word
//! sometimes emits — task 12's requirement.

use stem_core::ast::Block;

use super::super::xml::XmlBuf;
use super::ctx::{CaptionKind, EmitCtx};
use super::field;

/// Try to render `b` as one of the reserved TOC sections. Returns
/// `true` if the block was handled — the caller skips the
/// usual `section` recursion in that case.
pub fn try_render_toc_section(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) -> bool {
    let Some(id) = b.prop_str("id") else {
        return false;
    };
    match id {
        "toc" => {
            let (start, end) = parse_levels(b.prop_str("levels"));
            render_toc(start, end, ctx, x);
            true
        }
        "list-of-tables" | "lot" => {
            render_caption_index(CaptionKind::Table, ctx, x);
            true
        }
        "list-of-figures" | "lof" => {
            render_caption_index(CaptionKind::Figure, ctx, x);
            true
        }
        _ => false,
    }
}

/// Parse the optional `levels` property on `section[id:toc]`.
/// Accepts `"start-end"` (e.g. `"1-3"`) or a single value `"N"`
/// meaning `1-N`. Defaults to `1-3`.
fn parse_levels(spec: Option<&str>) -> (u32, u32) {
    let max = 6u32;
    let Some(s) = spec else { return (1, 3) };
    let s = s.trim();
    if let Some((lo, hi)) = s.split_once('-') {
        let lo = lo.trim().parse::<u32>().unwrap_or(1).clamp(1, max);
        let hi = hi.trim().parse::<u32>().unwrap_or(3).clamp(lo, max);
        return (lo, hi);
    }
    if let Ok(n) = s.parse::<u32>() {
        return (1, n.clamp(1, max));
    }
    (1, 3)
}

fn render_toc(start: u32, end: u32, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    // Header paragraph — centering comes from the TOCHeading style
    // itself (see parts/styles.rs), not from a per-paragraph
    // override. Matches the reference's "Contents Heading" style.
    x.elem("w:p", &[], |x| {
        x.elem("w:pPr", &[], |x| {
            x.empty("w:pStyle", &[("w:val", "TOCHeading")]);
        });
        x.elem("w:r", &[], |x| {
            x.elem_text("w:t", &[], "Table of Contents", false);
        });
    });

    let instr = format!(" TOC \\o \"{start}-{end}\" \\h \\z \\u ");
    let entries: Vec<_> = ctx
        .heading_anchors
        .iter()
        .filter(|h| h.level >= start && h.level <= end)
        .cloned()
        .collect();
    if entries.is_empty() {
        // Empty TOC — emit a single placeholder paragraph with the
        // field so Word can populate it on F9.
        x.elem("w:p", &[], |x| {
            x.elem("w:pPr", &[], |x| {
                x.empty("w:pStyle", &[("w:val", "TOC1")]);
            });
            field::render_complex(&instr, x, |_| {});
        });
        return;
    }

    for (i, entry) in entries.iter().enumerate() {
        let style = format!("TOC{}", entry.level.min(9));
        x.elem("w:p", &[], |x| {
            x.elem("w:pPr", &[], |x| {
                x.empty("w:pStyle", &[("w:val", &style)]);
                x.elem("w:tabs", &[], |x| {
                    x.empty(
                        "w:tab",
                        &[
                            ("w:val", "right"),
                            ("w:leader", "dot"),
                            ("w:pos", "9350"),
                        ],
                    );
                });
            });
            if i == 0 {
                // First entry carries the field's begin/instr/separate.
                x.elem("w:r", &[], |x| {
                    x.empty("w:fldChar", &[("w:fldCharType", "begin")]);
                });
                x.elem("w:r", &[], |x| {
                    x.elem_text("w:instrText", &[], &instr, true);
                });
                x.elem("w:r", &[], |x| {
                    x.empty("w:fldChar", &[("w:fldCharType", "separate")]);
                });
            }
            // Entry text + tab + page number, all inside an
            // anchor hyperlink.
            x.elem(
                "w:hyperlink",
                &[("w:anchor", &entry.bookmark), ("w:history", "1")],
                |x| {
                    x.elem("w:r", &[], |x| {
                        x.elem("w:rPr", &[], |x| {
                            x.empty("w:rStyle", &[("w:val", "Hyperlink")]);
                        });
                        x.elem_text("w:t", &[], &entry.text, true);
                    });
                    x.elem("w:r", &[], |x| {
                        x.elem_text("w:tab", &[], "", false);
                    });
                    field::render_page_ref(&entry.bookmark, "1", x);
                },
            );
            if i == entries.len() - 1 {
                // Last entry closes the field.
                x.elem("w:r", &[], |x| {
                    x.empty("w:fldChar", &[("w:fldCharType", "end")]);
                });
            }
        });
    }
}

fn render_caption_index(kind: CaptionKind, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    let (label, instr, header) = match kind {
        CaptionKind::Table => (
            "Table",
            r#" TOC \h \z \c "Table" "#,
            "List of Tables",
        ),
        CaptionKind::Figure => (
            "Figure",
            r#" TOC \h \z \c "Figure" "#,
            "List of Figures",
        ),
    };
    x.elem("w:p", &[], |x| {
        x.elem("w:pPr", &[], |x| {
            x.empty("w:pStyle", &[("w:val", "TOCHeading")]);
        });
        x.elem("w:r", &[], |x| {
            x.elem_text("w:t", &[], header, false);
        });
    });
    let entries: Vec<_> = ctx
        .captions
        .iter()
        .filter(|c| c.kind == kind)
        .cloned()
        .collect();
    if entries.is_empty() {
        x.elem("w:p", &[], |x| {
            x.elem("w:pPr", &[], |x| {
                x.empty("w:pStyle", &[("w:val", "TableofFigures")]);
            });
            field::render_complex(instr, x, |_| {});
        });
        return;
    }
    for (i, c) in entries.iter().enumerate() {
        x.elem("w:p", &[], |x| {
            x.elem("w:pPr", &[], |x| {
                x.empty("w:pStyle", &[("w:val", "TableofFigures")]);
            });
            if i == 0 {
                x.elem("w:r", &[], |x| {
                    x.empty("w:fldChar", &[("w:fldCharType", "begin")]);
                });
                x.elem("w:r", &[], |x| {
                    x.elem_text("w:instrText", &[], instr, true);
                });
                x.elem("w:r", &[], |x| {
                    x.empty("w:fldChar", &[("w:fldCharType", "separate")]);
                });
            }
            // "Table N. <text>" with hyperlink to the caption
            // anchor.
            let display = format!("{label} {}. {}", c.seq, c.text);
            x.elem(
                "w:hyperlink",
                &[("w:anchor", &c.bookmark), ("w:history", "1")],
                |x| {
                    x.elem("w:r", &[], |x| {
                        x.elem("w:rPr", &[], |x| {
                            x.empty("w:rStyle", &[("w:val", "Hyperlink")]);
                        });
                        x.elem_text("w:t", &[], &display, true);
                    });
                },
            );
            if i == entries.len() - 1 {
                x.elem("w:r", &[], |x| {
                    x.empty("w:fldChar", &[("w:fldCharType", "end")]);
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use stem_parser::parse;

    use super::super::ctx::{CaptionAnchor, HeadingAnchor};
    use super::*;

    fn ctx_with(headings: Vec<HeadingAnchor>, captions: Vec<CaptionAnchor>) -> EmitCtx {
        let mut ctx = EmitCtx::new(None, 1);
        ctx.heading_anchors = headings;
        ctx.captions = captions;
        ctx
    }

    fn first_block(src: &str) -> Block {
        parse(src).document.blocks.first().unwrap().clone()
    }

    #[test]
    fn parse_levels_accepts_range_single_and_default() {
        assert_eq!(parse_levels(None), (1, 3));
        assert_eq!(parse_levels(Some("2")), (1, 2));
        assert_eq!(parse_levels(Some("1-3")), (1, 3));
        assert_eq!(parse_levels(Some("2-5")), (2, 5));
        assert_eq!(parse_levels(Some("invalid")), (1, 3));
    }

    #[test]
    fn toc_with_no_headings_emits_field_placeholder() {
        let mut ctx = ctx_with(Vec::new(), Vec::new());
        let mut x = XmlBuf::new();
        try_render_toc_section(&first_block(r#"section[id:toc]"#), &mut ctx, &mut x);
        let s = x.finish();
        assert!(s.contains(r#"<w:pStyle w:val="TOCHeading"/>"#));
        assert!(s.contains("Table of Contents"));
        assert!(s.contains(" TOC "));
    }

    #[test]
    fn toc_pre_populates_entries_in_document_order() {
        let headings = vec![
            HeadingAnchor {
                bookmark: "_Toc1".into(),
                level: 1,
                text: "Alpha".into(),
            },
            HeadingAnchor {
                bookmark: "_Toc2".into(),
                level: 2,
                text: "Beta".into(),
            },
            HeadingAnchor {
                bookmark: "_Toc3".into(),
                level: 1,
                text: "Gamma".into(),
            },
        ];
        let mut ctx = ctx_with(headings, Vec::new());
        let mut x = XmlBuf::new();
        try_render_toc_section(&first_block(r#"section[id:toc]"#), &mut ctx, &mut x);
        let s = x.finish();
        // Three hyperlinks, one per heading.
        assert_eq!(s.matches("<w:hyperlink ").count(), 3);
        assert!(s.contains(r#"<w:hyperlink w:anchor="_Toc1""#));
        assert!(s.contains(r#"<w:hyperlink w:anchor="_Toc3""#));
        // First entry has fldChar begin; last entry has fldChar end.
        let begin = s.find(r#"w:fldCharType="begin""#).unwrap();
        let end = s.find(r#"w:fldCharType="end""#).unwrap();
        assert!(begin < end);
        // Visible text from headings.
        assert!(s.contains("Alpha") && s.contains("Beta") && s.contains("Gamma"));
        // Per-level TOC styles.
        assert!(s.contains(r#"<w:pStyle w:val="TOC1"/>"#));
        assert!(s.contains(r#"<w:pStyle w:val="TOC2"/>"#));
    }

    #[test]
    fn toc_respects_levels_range() {
        let headings = vec![
            HeadingAnchor {
                bookmark: "_Toc1".into(),
                level: 1,
                text: "Top".into(),
            },
            HeadingAnchor {
                bookmark: "_Toc2".into(),
                level: 3,
                text: "Deep".into(),
            },
        ];
        let mut ctx = ctx_with(headings, Vec::new());
        let mut x = XmlBuf::new();
        try_render_toc_section(
            &first_block(r#"section[id:toc, levels:"1-2"]"#),
            &mut ctx,
            &mut x,
        );
        let s = x.finish();
        // "Top" is included (level 1), "Deep" excluded (level 3).
        assert!(s.contains("Top"));
        assert!(!s.contains("Deep"));
    }

    #[test]
    fn list_of_tables_emits_TableofFigures_styled_entries() {
        let captions = vec![
            CaptionAnchor {
                kind: CaptionKind::Table,
                bookmark: "_Toc_table_1".into(),
                text: "Algorithms".into(),
                seq: 1,
            },
            CaptionAnchor {
                kind: CaptionKind::Figure,
                bookmark: "_Toc_figure_1".into(),
                text: "Diagram".into(),
                seq: 1,
            },
        ];
        let mut ctx = ctx_with(Vec::new(), captions);
        let mut x = XmlBuf::new();
        try_render_toc_section(
            &first_block(r#"section[id:list-of-tables]"#),
            &mut ctx,
            &mut x,
        );
        let s = x.finish();
        assert!(s.contains("List of Tables"));
        // Only Table entries are shown, not Figures.
        assert!(s.contains("Algorithms"));
        assert!(!s.contains("Diagram"));
        // Pre-formatted "Table 1. <text>".
        assert!(s.contains("Table 1. Algorithms"));
        // TableofFigures style applied.
        assert!(s.contains(r#"<w:pStyle w:val="TableofFigures"/>"#));
    }

    #[test]
    fn unknown_section_id_returns_false() {
        let mut ctx = ctx_with(Vec::new(), Vec::new());
        let mut x = XmlBuf::new();
        let handled = try_render_toc_section(
            &first_block(r#"section[id:other]"#),
            &mut ctx,
            &mut x,
        );
        assert!(!handled);
        assert_eq!(x.finish(), "");
    }
}
