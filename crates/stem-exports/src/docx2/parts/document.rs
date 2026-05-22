//! `word/document.xml` — the body of the docx.
//!
//! Task 1+2 emit a fixed minimal body (one empty paragraph + section
//! properties) so the file opens cleanly in Word. Real body emission
//! from the cooked AST lands in tasks 6 onward (paragraphs, runs,
//! tables, drawings, fields, hyperlinks, TOC).

use super::super::xml::{Ns, XmlBuf};

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// One empty paragraph wrapped in a body + section properties.
/// `sectPr` with `pgSz` and `pgMar` is required for Word to lay the
/// document out without prompting the user.
pub fn minimal() -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem_with_ns(
        "w:document",
        &[Ns { prefix: "w", uri: NS_W }],
        &[],
        |x| {
            x.elem("w:body", &[], |x| {
                x.empty("w:p", &[]);
                x.elem("w:sectPr", &[], |x| {
                    x.empty("w:pgSz", &[("w:w", "12240"), ("w:h", "15840")]);
                    x.empty(
                        "w:pgMar",
                        &[
                            ("w:top", "1440"),
                            ("w:right", "1440"),
                            ("w:bottom", "1440"),
                            ("w:left", "1440"),
                            ("w:header", "720"),
                            ("w:footer", "720"),
                            ("w:gutter", "0"),
                        ],
                    );
                });
            });
        },
    );
    x.finish()
}
