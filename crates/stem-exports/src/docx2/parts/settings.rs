//! `word/settings.xml` — document-level Word settings.
//!
//! What we need:
//! - Zoom level (Word stores the last open zoom; 100% is sensible).
//! - Default tab stop in twentieths of a point.
//! - Character spacing control (Word's default
//!   `doNotCompress` matches Office's modern behavior).
//! - `<w:footnotePr>` and `<w:endnotePr>` with the boilerplate -1/0
//!   placeholder IDs — required for footnotes/endnotes parts to
//!   resolve.
//! - `<w:compat>` block declaring Word 2013 compatibility mode so
//!   modern style behavior (line-spacing rules, leading vs trailing
//!   spacing) is consistent with what the reference target uses.
//!
//! What we deliberately leave out:
//! - `<w:rsids>` — Word's per-edit revision IDs. The reference docx
//!   has ~120 of them from years of editing; we have none. Word
//!   tolerates the absence; emitting a single rsidRoot would be
//!   fine too but adds no value.

use super::super::xml::XmlBuf;

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

pub fn settings() -> String {
    settings_with(false)
}

/// Settings part with optional `<w:evenAndOddHeaders/>` flag.
/// Word renders the "even" `w:headerReference` / `w:footerReference`
/// variants only when this flag is set.
pub fn settings_with(even_and_odd: bool) -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem("w:settings", &[("xmlns:w", NS_W)], |x| {
        x.empty("w:zoom", &[("w:percent", "100")]);
        x.empty("w:defaultTabStop", &[("w:val", "720")]);
        x.empty("w:characterSpacingControl", &[("w:val", "doNotCompress")]);
        if even_and_odd {
            x.empty("w:evenAndOddHeaders", &[]);
        }
        x.elem("w:footnotePr", &[], |x| {
            x.empty("w:footnote", &[("w:id", "-1")]);
            x.empty("w:footnote", &[("w:id", "0")]);
        });
        x.elem("w:endnotePr", &[], |x| {
            x.empty("w:endnote", &[("w:id", "-1")]);
            x.empty("w:endnote", &[("w:id", "0")]);
        });
        x.elem("w:compat", &[], |x| {
            for (name, val) in [
                ("compatibilityMode", "15"),
                ("overrideTableStyleFontSizeAndJustification", "1"),
                ("enableOpenTypeFeatures", "1"),
                ("doNotFlipMirrorIndents", "1"),
                ("differentiateMultirowTableHeaders", "1"),
            ] {
                x.empty(
                    "w:compatSetting",
                    &[
                        ("w:name", name),
                        ("w:uri", "http://schemas.microsoft.com/office/word"),
                        ("w:val", val),
                    ],
                );
            }
        });
    });
    x.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_has_required_blocks() {
        let s = settings();
        assert!(s.contains(r#"<w:zoom w:percent="100"/>"#));
        assert!(s.contains("<w:footnotePr>"));
        assert!(s.contains("<w:endnotePr>"));
        assert!(s.contains(r#"w:name="compatibilityMode""#));
        assert!(s.contains(r#"w:val="15""#));
    }
}
