//! `word/footnotes.xml` — the part holding the per-document
//! footnote contents that `<w:footnoteReference w:id="…"/>` in the
//! body refers to.
//!
//! Two boilerplate entries always go first:
//! - `id="-1"`, type `separator` — the line drawn above the
//!   footnotes block at the bottom of each page.
//! - `id="0"`, type `continuationSeparator` — drawn when a long
//!   footnote spills onto the next page.
//!
//! Then one entry per `EmitCtx::footnotes` registry entry.

use super::super::emit::ctx::FootnoteEntry;
use super::super::xml::{Ns, XmlBuf};

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

pub fn footnotes(entries: &[FootnoteEntry]) -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem_with_ns(
        "w:footnotes",
        &[Ns { prefix: "w", uri: NS_W }],
        &[],
        |x| {
            // Boilerplate separator entries — Word expects these.
            x.elem(
                "w:footnote",
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
                "w:footnote",
                &[("w:type", "continuationSeparator"), ("w:id", "0")],
                |x| {
                    x.elem("w:p", &[], |x| {
                        x.elem("w:r", &[], |x| {
                            x.empty("w:continuationSeparator", &[]);
                        });
                    });
                },
            );
            for entry in entries {
                let id_s = entry.id.to_string();
                x.elem("w:footnote", &[("w:id", &id_s)], |x| {
                    x.elem("w:p", &[], |x| {
                        // Reference run — Word draws this as the
                        // superscript number at the start of the
                        // footnote.
                        x.elem("w:r", &[], |x| {
                            x.elem("w:rPr", &[], |x| {
                                x.empty(
                                    "w:rStyle",
                                    &[("w:val", "FootnoteReference")],
                                );
                            });
                            x.empty("w:footnoteRef", &[]);
                        });
                        // Footnote body — plain text for task 14
                        // (rich inline runs are out of scope here;
                        // can be widened later by feeding through
                        // the same run dispatcher).
                        x.elem("w:r", &[], |x| {
                            x.elem_text(
                                "w:t",
                                &[],
                                &format!(" {}", entry.text),
                                true,
                            );
                        });
                    });
                });
            }
        },
    );
    x.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footnotes_with_no_entries_still_emits_separators() {
        let s = footnotes(&[]);
        assert!(s.contains(r#"<w:footnote w:type="separator" w:id="-1">"#));
        assert!(s.contains(r#"<w:footnote w:type="continuationSeparator" w:id="0">"#));
        // No user footnote entries.
        assert_eq!(s.matches(r#"<w:footnote w:id=""#).count(), 0);
    }

    #[test]
    fn one_footnote_entry_emits_with_reference_run() {
        let s = footnotes(&[FootnoteEntry {
            id: 1,
            text: "ipsum".into(),
        }]);
        assert!(s.contains(r#"<w:footnote w:id="1">"#));
        assert!(s.contains("<w:footnoteRef/>"));
        assert!(s.contains(r#"<w:t xml:space="preserve"> ipsum</w:t>"#));
    }
}
