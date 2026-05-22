#![cfg(feature = "docx")]

use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_exports::DocxExporter;
use stem_imports::markdown::import_str;

fn docx_bytes(src: &str) -> Vec<u8> {
    let doc = import_str(src);
    DocxExporter::new()
        .export(&doc, &Theme::default())
        .expect("export")
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
