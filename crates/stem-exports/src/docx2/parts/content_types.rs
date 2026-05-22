//! `[Content_Types].xml` — declares the MIME type of every part in
//! the package. Word refuses to open a docx whose parts aren't
//! registered here, so this list must stay in sync with what we
//! actually emit.

/// Content types for the minimal empty docx (task 1 scaffold).
/// Adds only the document part; subsequent tasks extend this with
/// styles, numbering, theme, header/footer, etc.
pub fn minimal() -> String {
    String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>
"#,
    )
}
