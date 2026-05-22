//! OPC relationship parts.
//!
//! Two layers:
//! - `_rels/.rels` — root, points at the main document part.
//! - `word/_rels/document.xml.rels` — document-level rels (styles,
//!   numbering, theme, settings, hyperlinks, etc.). Task 1 emits a
//!   placeholder with no document-scoped rels; later tasks fill it
//!   in as parts are added.

/// Root relationships file: declares which part is the main document.
pub fn root() -> String {
    String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
"#,
    )
}

/// Document-level relationships for the minimal scaffold (task 1).
/// Empty — styles/numbering/theme rels arrive in tasks 3-5.
pub fn document_minimal() -> String {
    String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>
"#,
    )
}
