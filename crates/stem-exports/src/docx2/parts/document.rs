//! `word/document.xml` — the body of the docx.
//!
//! Task 1 emits a fixed minimal body (one empty paragraph) so the
//! file opens cleanly in Word. Real body emission from the cooked
//! AST lands in tasks 6 onward (paragraphs, runs, tables, drawings,
//! fields, hyperlinks, TOC).

/// One empty paragraph wrapped in a body + section properties.
/// `sectPr` with `pgSz` and `pgMar` is required for Word to lay the
/// document out without prompting the user.
pub fn minimal() -> String {
    String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p/>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>
"#,
    )
}
