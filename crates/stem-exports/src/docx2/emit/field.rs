//! Field emission — PAGE, NUMPAGES, SEQ, PAGEREF, TOC.
//!
//! Two shapes:
//! - **Simple field**: `<w:fldSimple w:instr="…">…displayed…</w:fldSimple>`.
//!   Word accepts this for instructions without nested fields.
//!   We use it for `PAGE`, `NUMPAGES`, and `SEQ`.
//! - **Complex field**: three runs — `begin` / `instrText` /
//!   `separate` / displayed run / `end`. Needed when the field's
//!   displayed result contains formatting or nested fields (e.g.
//!   a TOC with hyperlinks). Task 12 uses this for `TOC`/`PAGEREF`.
//!
//! `<w:instrText>` always carries `xml:space="preserve"` and the
//! field instruction is wrapped in leading/trailing spaces — both
//! are what Word writes itself, and the docx-rs path got the
//! preserve attribute wrong in some emission paths.

use super::super::xml::XmlBuf;

/// Emit a `<w:fldSimple>` with a single displayed run carrying
/// `displayed` as plain text. `instr` is the field instruction
/// (e.g. ` PAGE `, ` NUMPAGES `).
pub fn render_simple(instr: &str, displayed: &str, x: &mut XmlBuf) {
    x.elem("w:fldSimple", &[("w:instr", instr)], |x| {
        x.elem("w:r", &[], |x| {
            x.elem_text("w:t", &[], displayed, true);
        });
    });
}

/// Emit a `PAGE` field. Wrapped in a single `<w:r>` so it slots
/// into a run sequence without extra paragraph nesting.
pub fn render_page(x: &mut XmlBuf) {
    render_simple(" PAGE   \\* MERGEFORMAT ", "1", x);
}

/// Emit a `NUMPAGES` field.
pub fn render_num_pages(x: &mut XmlBuf) {
    render_simple(" NUMPAGES   \\* MERGEFORMAT ", "1", x);
}

/// Emit a `SEQ <name>` field — used by table/figure captions to
/// produce auto-incrementing counters. `displayed` is the cached
/// number Word shows until the field is updated (F9).
pub fn render_seq(name: &str, displayed: u32, x: &mut XmlBuf) {
    let instr = format!(" SEQ {name} \\* ARABIC ");
    render_simple(&instr, &displayed.to_string(), x);
}

/// Emit a complex field with explicit begin/instr/separate/end
/// runs. The closure produces the displayed-result runs between
/// `separate` and `end`. Use for fields whose result needs nested
/// runs or hyperlinks (TOC, PAGEREF).
pub fn render_complex(instr: &str, x: &mut XmlBuf, result: impl FnOnce(&mut XmlBuf)) {
    x.elem("w:r", &[], |x| {
        x.empty("w:fldChar", &[("w:fldCharType", "begin")]);
    });
    x.elem("w:r", &[], |x| {
        x.elem_text("w:instrText", &[], instr, true);
    });
    x.elem("w:r", &[], |x| {
        x.empty("w:fldChar", &[("w:fldCharType", "separate")]);
    });
    result(x);
    x.elem("w:r", &[], |x| {
        x.empty("w:fldChar", &[("w:fldCharType", "end")]);
    });
}

/// Emit a `PAGEREF <bookmark>` field, used by TOC entries to
/// render the page number next to a TOC item.
pub fn render_page_ref(bookmark: &str, displayed: &str, x: &mut XmlBuf) {
    let instr = format!(" PAGEREF {bookmark} \\h ");
    render_complex(&instr, x, |x| {
        x.elem("w:r", &[], |x| {
            x.elem_text("w:t", &[], displayed, true);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_simple_field_emits_with_preserved_instr() {
        let mut x = XmlBuf::new();
        render_page(&mut x);
        let s = x.finish();
        assert!(
            s.starts_with(r#"<w:fldSimple w:instr=" PAGE   \* MERGEFORMAT "#),
            "got: {s}"
        );
        // The displayed run wraps a <w:t xml:space="preserve">.
        assert!(s.contains(r#"<w:t xml:space="preserve">1</w:t>"#));
    }

    #[test]
    fn numpages_simple_field_emits() {
        let mut x = XmlBuf::new();
        render_num_pages(&mut x);
        let s = x.finish();
        assert!(s.contains(" NUMPAGES "));
    }

    #[test]
    fn seq_emits_correct_instr_and_displayed_number() {
        let mut x = XmlBuf::new();
        render_seq("Table", 3, &mut x);
        let s = x.finish();
        assert!(s.contains(r#"w:instr=" SEQ Table \* ARABIC ""#));
        assert!(s.contains(r#"<w:t xml:space="preserve">3</w:t>"#));
    }

    #[test]
    fn complex_field_emits_begin_instr_separate_end_in_order() {
        let mut x = XmlBuf::new();
        render_complex(" TOC \\h \\z ", &mut x, |x| {
            x.elem("w:r", &[], |x| {
                x.elem_text("w:t", &[], "see TOC", false);
            });
        });
        let s = x.finish();
        let begin = s.find(r#"w:fldCharType="begin""#).unwrap();
        let instr = s.find("<w:instrText").unwrap();
        let sep = s.find(r#"w:fldCharType="separate""#).unwrap();
        let result = s.find("see TOC").unwrap();
        let end = s.find(r#"w:fldCharType="end""#).unwrap();
        assert!(begin < instr && instr < sep && sep < result && result < end);
        // instrText must carry xml:space="preserve".
        assert!(
            s.contains(r#"<w:instrText xml:space="preserve""#),
            "got: {s}"
        );
    }

    #[test]
    fn page_ref_emits_complex_with_bookmark_in_instr() {
        let mut x = XmlBuf::new();
        render_page_ref("_Toc1", "5", &mut x);
        let s = x.finish();
        assert!(s.contains("PAGEREF _Toc1"));
        assert!(s.contains(r#"<w:t xml:space="preserve">5</w:t>"#));
    }
}
