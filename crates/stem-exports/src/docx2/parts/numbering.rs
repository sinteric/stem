//! `word/numbering.xml` — bullet, ordered, and heading multilevel
//! list definitions.
//!
//! Three numIds:
//! - 1 → ordered list, 9 decimal levels.
//! - 2 → unordered list, 9 bullet levels.
//! - 3 → heading multilevel (1., 1.1, 1.1.1, ...) pStyle-linked
//!   to Heading1..6 so paragraphs styled `Heading1` automatically
//!   get the corresponding numbering when their `<w:numPr>` ties
//!   into this list.
//!
//! Schema order for `<w:lvl>` children (the bug that motivated
//! the migration here):
//!   start → numFmt → lvlRestart → pStyle → isLgl → suff →
//!   lvlText → lvlPicBulletId → legacy → lvlJc → pPr → rPr
//!
//! docx-rs put `<w:pStyle>` in the wrong slot, breaking the
//! Heading↔level link.

use super::super::xml::XmlBuf;

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// numId reserved for the heading multilevel list. 1+2 are used by
/// ordered/unordered lists; 3 is the heading list.
pub const NUM_ID_ORDERED: u32 = 1;
pub const NUM_ID_UNORDERED: u32 = 2;
pub const NUM_ID_HEADING: u32 = 3;

pub fn numbering() -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem("w:numbering", &[("xmlns:w", NS_W)], |x| {
        render_abstract_decimal(x, 1);
        render_abstract_bullet(x, 2);
        render_abstract_heading(x, 3);
        // <w:num> entries (must come after all <w:abstractNum>):
        render_num(x, NUM_ID_ORDERED, 1);
        render_num(x, NUM_ID_UNORDERED, 2);
        render_num(x, NUM_ID_HEADING, 3);
    });
    x.finish()
}

/// 9-level decimal list: "1.", "1.", "1.", ... (each level resets
/// to 1 at its own scope; Word handles the actual reset semantics).
fn render_abstract_decimal(x: &mut XmlBuf, abstract_id: u32) {
    let id_s = abstract_id.to_string();
    x.elem("w:abstractNum", &[("w:abstractNumId", &id_s)], |x| {
        x.empty("w:multiLevelType", &[("w:val", "hybridMultilevel")]);
        for lvl in 0..9u32 {
            render_lvl(x, lvl, LvlKind::Decimal, 720 * (lvl + 1), 360);
        }
    });
}

/// 9-level bullet list using the standard "•" glyph at every level.
fn render_abstract_bullet(x: &mut XmlBuf, abstract_id: u32) {
    let id_s = abstract_id.to_string();
    x.elem("w:abstractNum", &[("w:abstractNumId", &id_s)], |x| {
        x.empty("w:multiLevelType", &[("w:val", "hybridMultilevel")]);
        for lvl in 0..9u32 {
            render_lvl(x, lvl, LvlKind::Bullet, 720 * (lvl + 1), 360);
        }
    });
}

/// Heading multilevel: level 0 emits "1.", level 1 "1.1", level 2
/// "1.1.1", up to `MAX_HEADING_LEVEL`. pStyle-linked to Heading{N}
/// so Word draws the numbering automatically on heading paragraphs.
fn render_abstract_heading(x: &mut XmlBuf, abstract_id: u32) {
    let id_s = abstract_id.to_string();
    x.elem("w:abstractNum", &[("w:abstractNumId", &id_s)], |x| {
        x.empty("w:multiLevelType", &[("w:val", "multilevel")]);
        for lvl in 0..stem_types::MAX_HEADING_LEVEL as u32 {
            render_lvl(
                x,
                lvl,
                LvlKind::HeadingMultilevel { level: lvl },
                0,
                360,
            );
        }
    });
}

#[derive(Clone, Copy)]
enum LvlKind {
    Decimal,
    Bullet,
    HeadingMultilevel { level: u32 },
}

fn render_lvl(x: &mut XmlBuf, ilvl: u32, kind: LvlKind, ind_left: u32, hanging: u32) {
    let ilvl_s = ilvl.to_string();
    x.elem("w:lvl", &[("w:ilvl", &ilvl_s)], |x| {
        // 1. start
        x.empty("w:start", &[("w:val", "1")]);
        // 2. numFmt
        let fmt = match kind {
            LvlKind::Bullet => "bullet",
            LvlKind::Decimal | LvlKind::HeadingMultilevel { .. } => "decimal",
        };
        x.empty("w:numFmt", &[("w:val", fmt)]);
        // 4. pStyle (heading multilevel links each level to a style)
        if let LvlKind::HeadingMultilevel { level } = kind {
            let pstyle = format!("Heading{}", level + 1);
            x.empty("w:pStyle", &[("w:val", &pstyle)]);
        }
        // 7. lvlText
        let text = match kind {
            LvlKind::Bullet => "\u{2022}".to_string(),
            LvlKind::Decimal => format!("%{}.", ilvl + 1),
            LvlKind::HeadingMultilevel { level } => {
                let mut t = String::new();
                for i in 0..=level {
                    if i > 0 {
                        t.push('.');
                    }
                    t.push_str(&format!("%{}", i + 1));
                }
                // Top level gets a trailing dot ("1."), deeper
                // levels don't ("1.1" rather than "1.1.").
                if level == 0 {
                    t.push('.');
                }
                t
            }
        };
        x.empty("w:lvlText", &[("w:val", &text)]);
        // 10. lvlJc
        x.empty("w:lvlJc", &[("w:val", "left")]);
        // 11. pPr — indentation
        x.elem("w:pPr", &[], |x| {
            let left_s = ind_left.to_string();
            let hanging_s = hanging.to_string();
            x.empty(
                "w:ind",
                &[("w:left", &left_s), ("w:hanging", &hanging_s)],
            );
        });
        // 12. rPr — for bullet levels, set Symbol font hint so the
        // bullet glyph renders consistently across systems.
        if matches!(kind, LvlKind::Bullet) {
            x.elem("w:rPr", &[], |x| {
                x.empty("w:rFonts", &[("w:ascii", "Symbol"), ("w:hAnsi", "Symbol"), ("w:hint", "default")]);
            });
        }
    });
}

fn render_num(x: &mut XmlBuf, num_id: u32, abstract_id: u32) {
    let n = num_id.to_string();
    let a = abstract_id.to_string();
    x.elem("w:num", &[("w:numId", &n)], |x| {
        x.empty("w:abstractNumId", &[("w:val", &a)]);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbering_has_three_abstracts_and_three_nums() {
        let s = numbering();
        assert_eq!(s.matches("<w:abstractNum ").count(), 3);
        assert_eq!(s.matches("<w:num ").count(), 3);
    }

    #[test]
    fn lvl_child_order_is_canonical() {
        let s = numbering();
        // Find the first <w:lvl> block and assert child order.
        let lvl_start = s.find("<w:lvl ").unwrap();
        let lvl_end = s[lvl_start..].find("</w:lvl>").unwrap() + lvl_start;
        let block = &s[lvl_start..lvl_end];
        let start = block.find("<w:start").unwrap();
        let numfmt = block.find("<w:numFmt").unwrap();
        let lvltext = block.find("<w:lvlText").unwrap();
        let lvljc = block.find("<w:lvlJc").unwrap();
        let ppr = block.find("<w:pPr>").unwrap();
        assert!(start < numfmt, "{block}");
        assert!(numfmt < lvltext);
        assert!(lvltext < lvljc);
        assert!(lvljc < ppr);
    }

    #[test]
    fn heading_levels_have_pstyle_after_numfmt() {
        let s = numbering();
        // Find abstractNum 3 (heading multilevel).
        let h3 = s.find(r#"w:abstractNumId="3""#).unwrap();
        let end = s[h3..].find("</w:abstractNum>").unwrap() + h3;
        let block = &s[h3..end];
        // pStyle="Heading1" appears, and the schema order is
        // numFmt → pStyle → lvlText.
        for level in 1..=stem_types::MAX_HEADING_LEVEL {
            let id = format!(r#"w:val="Heading{level}""#);
            assert!(block.contains(&id), "missing {id} in heading numbering");
        }
        // Within the first <w:lvl> block, pStyle must come after
        // numFmt and before lvlText.
        let lvl_start = block.find("<w:lvl ").unwrap();
        let lvl_end = block[lvl_start..].find("</w:lvl>").unwrap() + lvl_start;
        let lvl = &block[lvl_start..lvl_end];
        let nf = lvl.find("<w:numFmt").unwrap();
        let ps = lvl.find("<w:pStyle").unwrap();
        let lt = lvl.find("<w:lvlText").unwrap();
        assert!(nf < ps && ps < lt, "{lvl}");
    }

    #[test]
    fn bullet_level_has_symbol_font_hint() {
        let s = numbering();
        // Bullet list is abstractNum 2.
        let pos = s.find(r#"w:abstractNumId="2""#).unwrap();
        let end = s[pos..].find("</w:abstractNum>").unwrap() + pos;
        let block = &s[pos..end];
        assert!(block.contains(r#"w:ascii="Symbol""#));
    }
}
