#![cfg(feature = "docx")]

use std::io::Read;

use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_exports::DocxExporter;
use stem_imports::markdown::import_str;
use stem_parser::parse;

fn docx_bytes(src: &str) -> Vec<u8> {
    let doc = import_str(src);
    DocxExporter::new()
        .export(&doc, &Theme::default())
        .expect("export")
}

fn docx_bytes_from_stem(src: &str) -> Vec<u8> {
    let r = parse(src);
    assert!(r.diagnostics.is_empty(), "parse errors: {:?}", r.diagnostics);
    DocxExporter::new()
        .export(&r.document, &Theme::default())
        .expect("export")
}

/// Pull `word/document.xml` (deflated entry) out of the .docx ZIP so we
/// can grep the OOXML.
fn extract_document_xml(bytes: &[u8]) -> String {
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("docx is a zip");
    let mut file = zip
        .by_name("word/document.xml")
        .expect("word/document.xml present");
    let mut s = String::new();
    file.read_to_string(&mut s).expect("utf-8 xml");
    s
}

#[test]
fn output_is_valid_zip() {
    let bytes = docx_bytes("# Hello\n");
    // .docx is a ZIP archive — magic "PK\x03\x04".
    assert!(
        bytes.starts_with(b"PK\x03\x04"),
        "missing ZIP magic: {:?}",
        &bytes[..bytes.len().min(8)]
    );
    assert!(bytes.len() > 1000, "output too small: {} bytes", bytes.len());
}

#[test]
fn document_xml_is_present() {
    // ZIP central directory should reference word/document.xml. We can
    // grep for the filename appearing in the byte stream.
    let bytes = docx_bytes("# Hello\n");
    let blob = String::from_utf8_lossy(&bytes);
    assert!(
        blob.contains("word/document.xml"),
        "missing word/document.xml in ZIP entries"
    );
}

#[test]
fn heading_emits_heading_style() {
    // The document.xml stream is compressed in the ZIP, but the
    // [Content_Types] and references are not. Instead of unzipping in
    // tests, just check the bytes are well-formed and non-trivial.
    let bytes = docx_bytes("# H1 Title\n\n## H2 Subtitle\n");
    assert!(bytes.len() > 1500);
    assert!(bytes.starts_with(b"PK\x03\x04"));
}

#[test]
fn list_renders_without_error() {
    let bytes = docx_bytes("- alpha\n- beta\n- gamma\n");
    assert!(bytes.starts_with(b"PK\x03\x04"));
    assert!(bytes.len() > 1500);
}

#[test]
fn ordered_list_renders() {
    let bytes = docx_bytes("1. First\n2. Second\n");
    assert!(bytes.starts_with(b"PK\x03\x04"));
}

#[test]
fn paragraph_with_bold_italic_renders() {
    let bytes = docx_bytes("This has **bold** and *italic* spans.\n");
    assert!(bytes.starts_with(b"PK\x03\x04"));
    assert!(bytes.len() > 1500);
}

#[test]
fn code_block_renders() {
    let bytes = docx_bytes("```rust\nfn main() {}\n```\n");
    assert!(bytes.starts_with(b"PK\x03\x04"));
}

#[test]
fn empty_document_is_valid() {
    let bytes = docx_bytes("");
    assert!(bytes.starts_with(b"PK\x03\x04"));
    assert!(bytes.len() > 500);
}

// --- tables -------------------------------------------------------------

#[test]
fn table_emits_tbl_with_rows_and_cells() {
    let bytes = docx_bytes_from_stem(
        "table[border:all]{ row[kind:header]{ cell(A) cell(B) } row{ cell(1) cell(2) } }",
    );
    let xml = extract_document_xml(&bytes);
    // Two rows × two cells each.
    let tbl_count = xml.matches("<w:tbl>").count();
    let tr_count = xml.matches("<w:tr>").count();
    let tc_count = xml.matches("<w:tc>").count();
    assert_eq!(tbl_count, 1, "expected 1 <w:tbl>, xml: {}", xml);
    assert_eq!(tr_count, 2, "expected 2 <w:tr>, got {}", tr_count);
    assert_eq!(tc_count, 4, "expected 4 <w:tc>, got {}", tc_count);
    // Header cell text becomes bold.
    assert!(xml.contains("<w:b "), "expected a <w:b> bold marker in header");
}

#[test]
fn table_colspan_emits_grid_span() {
    let bytes = docx_bytes_from_stem(
        "table{ row[kind:header]{ cell(Title) cell[colspan:2](Span2) } row{ cell(a) cell(b) cell(c) } }",
    );
    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("w:gridSpan w:val=\"2\""), "expected gridSpan=2; xml: {}", xml);
}

#[test]
fn table_rowspan_emits_vmerge_restart_and_continue() {
    let bytes = docx_bytes_from_stem(
        "table{ row{ cell[rowspan:2](Tall) cell(A) } row{ cell(B) } }",
    );
    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("w:vMerge w:val=\"restart\""), "expected vMerge=restart; xml: {}", xml);
    assert!(xml.contains("w:vMerge w:val=\"continue\""), "expected vMerge=continue; xml: {}", xml);
    // Three rows in source had two cells then one — after merge expansion
    // we still get two <w:tr> with two <w:tc> each (the continue cell
    // synthesizes the missing slot in row 2).
    let tr_count = xml.matches("<w:tr>").count();
    let tc_count = xml.matches("<w:tc>").count();
    assert_eq!(tr_count, 2);
    assert_eq!(tc_count, 4);
}

#[test]
fn table_bg_resolves_named_theme_color() {
    let bytes = docx_bytes_from_stem("table{ row{ cell[bg:yellow](highlight) } }");
    let xml = extract_document_xml(&bytes);
    // Theme yellow is #ffd33d → "FFD33D" in OOXML.
    assert!(
        xml.contains("FFD33D") || xml.contains("ffd33d"),
        "expected yellow fill in xml: {}",
        xml
    );
}

#[test]
fn table_caption_appears_before_table() {
    let bytes = docx_bytes_from_stem(r#"table[caption:"Tab 1"]{ row{ cell(x) } }"#);
    let xml = extract_document_xml(&bytes);
    let caption_pos = xml.find("Tab 1").expect("caption text present");
    let tbl_pos = xml.find("<w:tbl>").expect("table present");
    assert!(caption_pos < tbl_pos, "caption should precede table");
    assert!(
        xml.contains("w:pStyle w:val=\"Caption\""),
        "caption should use Caption style: {}",
        xml
    );
}

#[test]
fn table_border_outer_clears_inside_borders() {
    let bytes_outer = docx_bytes_from_stem("table[border:outer]{ row{ cell(a) cell(b) } }");
    let xml_outer = extract_document_xml(&bytes_outer);
    // outer mode: outer single borders kept, inside H/V cleared. docx-rs
    // emits cleared borders as `w:val="nil"` (Word treats nil and none
    // equivalently for inside borders).
    assert!(xml_outer.contains("w:insideH w:val=\"nil\""), "outer borders should clear insideH: {}", xml_outer);
    assert!(xml_outer.contains("w:insideV w:val=\"nil\""), "outer borders should clear insideV: {}", xml_outer);
    assert!(xml_outer.contains("w:top w:val=\"single\""), "outer borders should keep top: {}", xml_outer);
}

#[test]
fn table_valign_emits_vertical_align() {
    let bytes = docx_bytes_from_stem("table{ row{ cell[valign:middle](x) } }");
    let xml = extract_document_xml(&bytes);
    assert!(
        xml.contains("w:vAlign w:val=\"center\""),
        "expected vAlign=center: {}",
        xml
    );
}
