//! `word/endnotes.xml` — sibling to `footnotes.xml` for the
//! end-of-document note kind.
//!
//! Stem doesn't yet expose an `@endnote(...)` inline, but Word's
//! settings.xml references the two placeholder endnote ids (-1
//! and 0) — so the part has to exist or Word reports a corrupted
//! document on open. We always emit the separator boilerplate;
//! user endnotes can land here later.

use super::super::xml::{Ns, XmlBuf};

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

pub fn endnotes() -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem_with_ns(
        "w:endnotes",
        &[Ns { prefix: "w", uri: NS_W }],
        &[],
        |x| {
            x.elem(
                "w:endnote",
                &[("w:type", "separator"), ("w:id", "-1")],
                |x| {
                    x.elem("w:p", &[], |x| {
                        x.elem("w:r", &[], |x| {
                            x.empty("w:separator", &[]);
                        });
                    });
                },
            );
            x.elem(
                "w:endnote",
                &[("w:type", "continuationSeparator"), ("w:id", "0")],
                |x| {
                    x.elem("w:p", &[], |x| {
                        x.elem("w:r", &[], |x| {
                            x.empty("w:continuationSeparator", &[]);
                        });
                    });
                },
            );
        },
    );
    x.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endnotes_has_both_separator_entries() {
        let s = endnotes();
        assert!(s.contains(r#"<w:endnote w:type="separator" w:id="-1">"#));
        assert!(s.contains(r#"<w:endnote w:type="continuationSeparator" w:id="0">"#));
        assert!(s.contains("<w:separator/>"));
        assert!(s.contains("<w:continuationSeparator/>"));
    }
}
