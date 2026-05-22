//! OPC relationship parts.
//!
//! Two layers:
//! - `_rels/.rels` — root, points at the main document part.
//! - `word/_rels/document.xml.rels` — document-level rels (styles,
//!   numbering, theme, settings, hyperlinks, etc.).
//! - Per-part rels (`word/_rels/headerN.xml.rels`, etc.) — emitted
//!   alongside their owning part in later tasks.

use super::super::xml::XmlBuf;

const NS_OPC_REL: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships";

pub mod kind {
    //! Canonical `Type` URIs for the relationships we'll emit.
    pub const OFFICE_DOC: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
    pub const STYLES: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
    pub const NUMBERING: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
    pub const THEME: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
    pub const SETTINGS: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
    pub const WEB_SETTINGS: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/webSettings";
    pub const FONT_TABLE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable";
    pub const FOOTNOTES: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
    pub const ENDNOTES: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
    pub const HEADER: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
    pub const FOOTER: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
    pub const HYPERLINK: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
    pub const IMAGE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
    pub const CORE_PROPS: &str =
        "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
    pub const EXTENDED_PROPS: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
}

/// One relationship entry: `<Relationship Id="rIdN" Type="..." Target="..."/>`.
pub struct Rel {
    pub id: String,
    pub kind: &'static str,
    pub target: String,
    /// `External` for hyperlinks to outside URIs; `None` (the
    /// default) for in-package targets.
    pub target_mode_external: bool,
}

impl Rel {
    pub fn new(id: impl Into<String>, kind: &'static str, target: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            target: target.into(),
            target_mode_external: false,
        }
    }
    pub fn external(id: impl Into<String>, kind: &'static str, target: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            target: target.into(),
            target_mode_external: true,
        }
    }
}

/// Build a relationships part from a list of [`Rel`]s.
pub fn build(rels: &[Rel]) -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem("Relationships", &[("xmlns", NS_OPC_REL)], |x| {
        for r in rels {
            let mut attrs: Vec<(&str, &str)> =
                vec![("Id", &r.id), ("Type", r.kind), ("Target", &r.target)];
            if r.target_mode_external {
                attrs.push(("TargetMode", "External"));
            }
            x.empty("Relationship", &attrs);
        }
    });
    x.finish()
}

/// Root relationships file: declares which part is the main document.
pub fn root() -> String {
    build(&[Rel::new("rId1", kind::OFFICE_DOC, "word/document.xml")])
}

/// Root rels including the docProps refs. The reference docx puts
/// the office-document rel first, then the docProps rels — keep
/// that order so a structural diff against the reference is clean.
pub fn root_with_metadata() -> String {
    build(&[
        Rel::new("rId1", kind::OFFICE_DOC, "word/document.xml"),
        Rel::new("rId2", kind::CORE_PROPS, "docProps/core.xml"),
        Rel::new("rId3", kind::EXTENDED_PROPS, "docProps/app.xml"),
    ])
}

/// Document-level relationships for the minimal scaffold (task 1).
/// Empty — styles/numbering/theme rels are added in tasks 3-5 by
/// callers that build their own `Vec<Rel>` and pass it to [`build`].
pub fn document_minimal() -> String {
    build(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_rels_includes_office_doc() {
        let s = root();
        assert!(s.contains(r#"Id="rId1""#));
        assert!(s.contains(r#"Target="word/document.xml""#));
        assert!(s.contains("officeDocument"));
    }

    #[test]
    fn external_target_mode_emits() {
        let s = build(&[Rel::external("rId7", kind::HYPERLINK, "https://example.org")]);
        assert!(s.contains(r#"TargetMode="External""#));
        assert!(s.contains(r#"Target="https://example.org""#));
    }
}
