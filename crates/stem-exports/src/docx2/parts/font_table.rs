//! `word/fontTable.xml` — declares the fonts referenced anywhere in
//! the document or its styles.
//!
//! Word doesn't strictly require fonts to be registered here (it'll
//! substitute system equivalents), but registering them gives the
//! best fidelity on machines without the font installed: panose1 +
//! signature bits let Word pick a close-enough fallback.
//!
//! The list mirrors the BoringCrypto reference's font usage: the
//! Office defaults (Calibri / Calibri Light / Cambria) plus the
//! workhorse families styles reference (Times New Roman, Courier
//! New, Arial, Symbol).

use super::super::xml::XmlBuf;

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

struct FontEntry {
    name: &'static str,
    panose1: Option<&'static str>,
    charset: &'static str,
    family: &'static str,
    pitch: &'static str,
    sig: Option<FontSig>,
}

struct FontSig {
    usb0: &'static str,
    usb1: &'static str,
    usb2: &'static str,
    usb3: &'static str,
    csb0: &'static str,
    csb1: &'static str,
}

pub fn font_table() -> String {
    let entries: &[FontEntry] = &[
        FontEntry {
            name: "Times New Roman",
            panose1: Some("02020603050405020304"),
            charset: "00",
            family: "roman",
            pitch: "variable",
            sig: Some(FontSig {
                usb0: "E0002EFF",
                usb1: "C000785B",
                usb2: "00000009",
                usb3: "00000000",
                csb0: "000001FF",
                csb1: "00000000",
            }),
        },
        FontEntry {
            name: "Symbol",
            panose1: Some("05050102010706020507"),
            charset: "02",
            family: "roman",
            pitch: "variable",
            sig: Some(FontSig {
                usb0: "00000000",
                usb1: "10000000",
                usb2: "00000000",
                usb3: "00000000",
                csb0: "80000000",
                csb1: "00000000",
            }),
        },
        FontEntry {
            name: "Courier New",
            panose1: Some("02070309020205020404"),
            charset: "00",
            family: "modern",
            pitch: "fixed",
            sig: Some(FontSig {
                usb0: "E0002EFF",
                usb1: "C0007843",
                usb2: "00000009",
                usb3: "00000000",
                csb0: "000001FF",
                csb1: "00000000",
            }),
        },
        FontEntry {
            name: "Calibri",
            panose1: Some("020F0502020204030204"),
            charset: "00",
            family: "swiss",
            pitch: "variable",
            sig: Some(FontSig {
                usb0: "E0002AFF",
                usb1: "C000247B",
                usb2: "00000009",
                usb3: "00000000",
                csb0: "000001FF",
                csb1: "00000000",
            }),
        },
        FontEntry {
            name: "Calibri Light",
            panose1: Some("020F0302020204030204"),
            charset: "00",
            family: "swiss",
            pitch: "variable",
            sig: Some(FontSig {
                usb0: "A0002AEF",
                usb1: "4000207B",
                usb2: "00000000",
                usb3: "00000000",
                csb0: "000001FF",
                csb1: "00000000",
            }),
        },
        FontEntry {
            name: "Arial",
            panose1: Some("020B0604020202020204"),
            charset: "00",
            family: "swiss",
            pitch: "variable",
            sig: Some(FontSig {
                usb0: "E0002EFF",
                usb1: "C000785B",
                usb2: "00000009",
                usb3: "00000000",
                csb0: "000001FF",
                csb1: "00000000",
            }),
        },
        FontEntry {
            name: "Cambria",
            panose1: Some("02040503050406030204"),
            charset: "00",
            family: "roman",
            pitch: "variable",
            sig: Some(FontSig {
                usb0: "E00006FF",
                usb1: "420024FF",
                usb2: "02000000",
                usb3: "00000000",
                csb0: "0000019F",
                csb1: "00000000",
            }),
        },
    ];

    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem("w:fonts", &[("xmlns:w", NS_W)], |x| {
        for e in entries {
            x.elem("w:font", &[("w:name", e.name)], |x| {
                if let Some(p) = e.panose1 {
                    x.empty("w:panose1", &[("w:val", p)]);
                }
                x.empty("w:charset", &[("w:val", e.charset)]);
                x.empty("w:family", &[("w:val", e.family)]);
                x.empty("w:pitch", &[("w:val", e.pitch)]);
                if let Some(sig) = &e.sig {
                    x.empty(
                        "w:sig",
                        &[
                            ("w:usb0", sig.usb0),
                            ("w:usb1", sig.usb1),
                            ("w:usb2", sig.usb2),
                            ("w:usb3", sig.usb3),
                            ("w:csb0", sig.csb0),
                            ("w:csb1", sig.csb1),
                        ],
                    );
                }
            });
        }
    });
    x.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_table_includes_standard_families() {
        let s = font_table();
        for name in [
            "Times New Roman",
            "Symbol",
            "Courier New",
            "Calibri",
            "Calibri Light",
            "Arial",
            "Cambria",
        ] {
            assert!(
                s.contains(&format!(r#"w:name="{name}""#)),
                "missing font {name} in {s}"
            );
        }
    }
}
