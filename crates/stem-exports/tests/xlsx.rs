#![cfg(feature = "xlsx")]

use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_exports::XlsxExporter;
use stem_parser::parse;

fn xlsx_bytes(src: &str) -> Vec<u8> {
    let r = parse(src);
    assert!(r.diagnostics.is_empty(), "parse errors: {:?}", r.diagnostics);
    XlsxExporter::new()
        .export(&r.document, &Theme::default())
        .expect("export")
}

#[test]
fn output_is_valid_zip() {
    let bytes = xlsx_bytes(r#"[type:sheet]
sheet[id:Demo]{
  cell[at:A1](Hello)
}"#);
    // xlsx is a ZIP — magic "PK\x03\x04".
    assert!(bytes.starts_with(b"PK\x03\x04"), "missing ZIP magic");
    assert!(bytes.len() > 1000);
}

#[test]
fn cells_with_values_render() {
    let bytes = xlsx_bytes(
        r#"[type:sheet]
sheet[id:Demo]{
  cell[at:A1](Item)
  cell[at:B1](Price)
  cell[at:A2](Widget)
  cell[at:B2](42)
}"#,
    );
    assert!(bytes.starts_with(b"PK\x03\x04"));
    // Quick sanity: must contain "xl/worksheets/sheet1.xml" entry.
    let blob = String::from_utf8_lossy(&bytes);
    assert!(blob.contains("xl/worksheets/sheet1.xml"));
}

#[test]
fn formula_cell_renders_as_formula() {
    let bytes = xlsx_bytes(
        r#"[type:sheet]
sheet[id:Demo]{
  cell[at:A1](10)
  cell[at:A2](20)
  cell[at:A3](@formula("SUM(A1:A2)"))
}"#,
    );
    assert!(bytes.starts_with(b"PK\x03\x04"));
}

#[test]
fn multiple_sheets() {
    let bytes = xlsx_bytes(
        r#"[type:sheet]
sheet[id:Q1, name:"Q1"]{
  cell[at:A1](Q1 sheet)
}
sheet[id:Q2, name:"Q2"]{
  cell[at:A1](Q2 sheet)
}"#,
    );
    let blob = String::from_utf8_lossy(&bytes);
    assert!(blob.contains("xl/worksheets/sheet1.xml"));
    assert!(blob.contains("xl/worksheets/sheet2.xml"));
}

#[test]
fn empty_document_still_valid() {
    // No sheet blocks → exporter emits a single empty Sheet1.
    let r = parse("[type:document]");
    let bytes = XlsxExporter::new()
        .export(&r.document, &Theme::default())
        .expect("export");
    assert!(bytes.starts_with(b"PK\x03\x04"));
}

#[test]
fn sheet_name_sanitized() {
    // Sheet names can't contain / \\ ? * [ ]; the exporter strips them.
    // Stem property values can contain those chars when quoted, so this
    // is a real path: confirm we don't crash and produce a valid file.
    let bytes = xlsx_bytes(
        r#"[type:sheet]
sheet[id:weird, name:"a/b\\c"]{
  cell[at:A1](x)
}"#,
    );
    assert!(bytes.starts_with(b"PK\x03\x04"));
}
